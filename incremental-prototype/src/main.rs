use serde::Serialize;
use stack_graphs_incremental_prototype::{
    facts_for, AnalysisHost, DocumentProjection, PublishedFact, Resolution, Summary,
};
use std::{collections::BTreeMap, fs, path::PathBuf};

const MODULES: [&str; 5] = ["a", "b", "unrelated", "fallback", "ghost"];
const FILES: [&str; 5] = ["a.py", "b.py", "unrelated.py", "fallback.py", "ghost.py"];
const A: &str = "class Left:\n    def value(self):\n        return 'left'\n\nclass Right:\n    def value(self):\n        return 'right'\n\ndef identity(x):\n    return x\n\ndef pick(x):\n    return x\n\ndef cycle_a(flag):\n    return 'seed' if flag else cycle_b()\n\ndef cycle_b():\n    return cycle_a(True)\n\ndef twin():\n    return 1\n\ndef twin():\n    return 2\n";
const B: &str = "from a import pick as chosen\nfrom a import identity as ident\nfrom a import Left, Right\nfrom ghost import later\nleft = ident(Left())\nright = ident(Right())\nleft_value = Left().value()\nright_value = Right().value()\nvalue = chosen(1)\n";

#[derive(Clone, Serialize)]
struct Snapshot {
    summaries: BTreeMap<String, Summary>,
    projections: BTreeMap<String, Option<DocumentProjection>>,
    resolutions: Vec<Resolution>,
    facts: Vec<PublishedFact>,
}
#[derive(Serialize)]
struct Report {
    status: String,
    edits: usize,
    expected: Vec<String>,
    salsa_event_executions: BTreeMap<String, usize>,
    query_body_executions: BTreeMap<String, usize>,
    memo_validations: BTreeMap<String, usize>,
    final_facts: Vec<PublishedFact>,
}

fn initial_inputs() -> Vec<(&'static str, &'static str, String)> {
    vec![
        ("a.py", "a", A.into()),
        ("b.py", "b", B.into()),
        (
            "unrelated.py",
            "unrelated",
            "def untouched():\n    return 0\n".into(),
        ),
        (
            "fallback.py",
            "fallback",
            "def pick(x):\n    return 2\n".into(),
        ),
    ]
}
fn host_from(inputs: &[(&'static str, &'static str, String)]) -> AnalysisHost {
    let mut host = AnalysisHost::new().unwrap();
    for (file, module, source) in inputs {
        host.set_document(file, module, source);
    }
    host
}
fn snapshot(host: &mut AnalysisHost) -> Snapshot {
    let summaries = MODULES
        .iter()
        .map(|m| ((*m).to_string(), host.summary(m)))
        .collect();
    let projections = FILES
        .iter()
        .map(|f| ((*f).to_string(), host.document_projection(f)))
        .collect();
    let b = host.summary("b");
    let chosen = b
        .references
        .iter()
        .find(|r| r.name == "chosen")
        .cloned()
        .unwrap_or_else(|| panic!("lowerer did not produce call reference for chosen"));
    let id_calls: Vec<_> = b
        .references
        .iter()
        .filter(|r| r.name == "ident")
        .cloned()
        .collect();
    let left = id_calls
        .iter()
        .find(|r| r.context.contains("Left"))
        .expect("Left call reference");
    let right = id_calls
        .iter()
        .find(|r| r.context.contains("Right"))
        .expect("Right call reference");
    let members: Vec<_> = b.references.iter().filter(|r| r.name == "value").collect();
    let left_member = members
        .iter()
        .find(|r| r.context == "receiver:class:Left")
        .expect("lowerer-derived Left receiver");
    let right_member = members
        .iter()
        .find(|r| r.context == "receiver:class:Right")
        .expect("lowerer-derived Right receiver");
    let resolutions = vec![
        host.resolve_name("b", &chosen.id, &chosen.name, &chosen.context),
        host.resolve_name("b", "ref:missing-module-symbol", "later", "plain"),
        host.resolve_name("a", "ref:competing", "twin", "plain"),
        host.resolve_name(
            "b",
            &left_member.id,
            &left_member.name,
            &left_member.context,
        ),
        host.resolve_name(
            "b",
            &right_member.id,
            &right_member.name,
            &right_member.context,
        ),
        host.resolve_name("b", &left.id, &left.name, &left.context),
        host.resolve_name("b", &right.id, &right.name, &right.context),
    ];
    let facts = facts_for(host, &MODULES);
    Snapshot {
        summaries,
        projections,
        resolutions,
        facts,
    }
}
fn check(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}
fn compare_fresh(
    host: &mut AnalysisHost,
    inputs: &[(&'static str, &'static str, String)],
    edit: &str,
) -> Result<Snapshot, String> {
    let actual = snapshot(host);
    let mut fresh = host_from(inputs);
    let expected = snapshot(&mut fresh);
    check(
        actual.summaries == expected.summaries,
        &format!("{edit}: summaries differ from fresh host"),
    )?;
    check(
        actual.projections == expected.projections,
        &format!("{edit}: document facts or source ranges differ from fresh host"),
    )?;
    check(
        actual.resolutions == expected.resolutions,
        &format!("{edit}: targets, candidates, behavior, or completeness differ from fresh host"),
    )?;
    check(
        actual.facts == expected.facts,
        &format!("{edit}: relationship facts differ from fresh host"),
    )?;
    Ok(actual)
}
fn replace(
    host: &mut AnalysisHost,
    inputs: &mut [(&'static str, &'static str, String)],
    file: &str,
    source: String,
    label: &str,
) -> Result<Snapshot, String> {
    let entry = inputs
        .iter_mut()
        .find(|r| r.0 == file)
        .ok_or_else(|| format!("unknown document {file}"))?;
    entry.2 = source;
    host.set_document(entry.0, entry.1, &entry.2);
    compare_fresh(host, inputs, label)
}
fn publish(host: &mut AnalysisHost) -> Result<Vec<PublishedFact>, String> {
    let roots = host.discover_all_documents();
    check(
        roots.len() == host.document_count(),
        "publication did not discover every current document root",
    )?;
    let facts = facts_for(host, &MODULES);
    let supports: Vec<_> = MODULES.iter().map(|m| format!("module:{m}")).collect();
    host.publish_supports(host.revision(), &supports, &facts, false, None)?;
    check(
        host.published_facts().map_err(|e| e.to_string())? == facts,
        "published SQLite facts differ from current semantic projection",
    )?;
    Ok(facts)
}
fn add_document(
    host: &mut AnalysisHost,
    inputs: &mut Vec<(&'static str, &'static str, String)>,
    file: &'static str,
    module: &'static str,
    source: String,
    label: &str,
) -> Result<Snapshot, String> {
    inputs.push((file, module, source.clone()));
    host.set_document(file, module, &source);
    compare_fresh(host, inputs, label)
}
fn delete_document(
    host: &mut AnalysisHost,
    inputs: &mut Vec<(&'static str, &'static str, String)>,
    file: &str,
    label: &str,
) -> Result<Snapshot, String> {
    host.remove_document(file);
    inputs.retain(|r| r.0 != file);
    compare_fresh(host, inputs, label)
}

fn run() -> Result<Report, String> {
    // Publish an importer after its provider changes, before any importer query.
    let mut untouched_inputs = initial_inputs();
    let b_input = untouched_inputs.iter_mut().find(|r| r.0 == "b.py").unwrap();
    b_input.2.push_str("from fallback import pick as chosen\n");
    let mut untouched = host_from(&untouched_inputs);
    let preferred_id = format!("{}", A);
    let provider_update = preferred_id.replace("def pick(x):\n    return x\n", "");
    untouched.set_document("a.py", "a", &provider_update);
    let untouched_facts = publish(&mut untouched)?;
    untouched_inputs
        .iter_mut()
        .find(|r| r.0 == "a.py")
        .unwrap()
        .2 = provider_update;
    let mut untouched_fresh = host_from(&untouched_inputs);
    let fresh_untouched_facts = publish(&mut untouched_fresh)?;
    check(
        untouched_facts == fresh_untouched_facts,
        "publication missed importer changes without earlier editor queries",
    )?;
    let importer_targets: Vec<_> = untouched_facts
        .iter()
        .filter(|fact| fact.subject == "b:chosen" && fact.predicate == "imports")
        .map(|fact| fact.object.clone())
        .collect();
    check(
        importer_targets
            .iter()
            .any(|target| target.contains("fallback")),
        "unqueried importer publication did not select the fallback provider",
    )?;

    let mut host = AnalysisHost::new().map_err(|e| e.to_string())?;
    let mut current = initial_inputs();
    for (file, module, source) in &current {
        host.set_document(file, module, source);
    }
    let mut edits = 0;
    let initial = compare_fresh(&mut host, &current, "initial")?;
    check(
        initial.resolutions[0].outcome == "unique",
        "expected unique imported name",
    )?;
    check(
        initial.resolutions[0].behavior
            == vec![stack_graphs_incremental_prototype::resolver::Value::Literal("1".into())],
        "lowerer-derived call context did not preserve the argument",
    )?;
    check(
        initial.resolutions[1].outcome == "missing",
        "expected tracked missing-module lookup",
    )?;
    check(
        initial.resolutions[2].outcome == "competing",
        &format!(
            "expected competing declaration lookup; outcome={}, candidates={:?}",
            initial.resolutions[2].outcome, initial.resolutions[2].candidates
        ),
    )?;
    check(
        initial.resolutions[3].targets.len() == 1
            && initial.resolutions[4].targets.len() == 1
            && initial.resolutions[3].targets != initial.resolutions[4].targets,
        "receiver-specific members did not select distinct class methods",
    )?;
    check(
        initial.resolutions[5].behavior != initial.resolutions[6].behavior,
        "identity(Left()) and identity(Right()) collapsed to one summary result",
    )?;
    let unrelated_initial = host
        .body_counts()
        .get("module:unrelated")
        .copied()
        .unwrap_or(0);
    publish(&mut host)?;

    // Add a lower-precedence import. Keep the selected target and answer equal.
    let b_with_fallback = format!(
        "{}from fallback import pick as chosen\n",
        current.iter().find(|r| r.0 == "b.py").unwrap().2
    );
    let snap = replace(
        &mut host,
        &mut current,
        "b.py",
        b_with_fallback,
        "add lower-precedence import",
    )?;
    check(
        snap.resolutions[0].targets == initial.resolutions[0].targets,
        "lower-precedence import changed the chosen target",
    )?;
    let consumer_before = host.observable_resolution("b", "chosen", "arg:literal:1");
    let consumer_count = host
        .body_counts()
        .get("observable:b:chosen:arg:literal:1")
        .copied()
        .unwrap_or(0);
    publish(&mut host)?;
    edits += 1;

    // Change the newly observed lower-precedence dependency. The chosen answer stays equal.
    replace(
        &mut host,
        &mut current,
        "fallback.py",
        "def pick(x):\n    return 200\n".into(),
        "change lower-precedence module",
    )?;
    check(
        host.observable_resolution("b", "chosen", "arg:literal:1") == consumer_before,
        "lower-precedence change altered selected answer",
    )?;
    let chosen_id = host
        .summary("b")
        .references
        .iter()
        .find(|r| r.name == "chosen")
        .unwrap()
        .id
        .clone();
    let chosen_resolver_key = format!("resolve:b:{chosen_id}:chosen:arg:literal:1");
    check(
        host.body_counts()
            .get(&chosen_resolver_key)
            .copied()
            .unwrap_or(0)
            > 0,
        "resolver body instrumentation missing",
    )?;
    check(
        host.body_counts()
            .get("observable:b:chosen:arg:literal:1")
            .copied()
            .unwrap_or(0)
            == consumer_count,
        "equal selected answer recomputed its consumer",
    )?;
    publish(&mut host)?;
    edits += 1;

    // Add an unrelated export in the selected module. Its projection changes, but the lookup answer does not.
    let a = current.iter().find(|r| r.0 == "a.py").unwrap().2.clone();
    let with_extra = format!("def extra():\n    return 99\n\n{a}");
    replace(
        &mut host,
        &mut current,
        "a.py",
        with_extra,
        "add unrelated export",
    )?;
    check(
        host.observable_resolution("b", "chosen", "arg:literal:1") == consumer_before,
        "unrelated export changed selected answer",
    )?;
    check(
        host.body_counts()
            .get("observable:b:chosen:arg:literal:1")
            .copied()
            .unwrap_or(0)
            == consumer_count,
        "unchanged selected answer recomputed consumer",
    )?;
    publish(&mut host)?;
    edits += 1;

    // Rewrite the function body through a local alias. Its symbolic summary remains Argument(0).
    let a = current.iter().find(|r| r.0 == "a.py").unwrap().2.replace(
        "def pick(x):\n    return x",
        "def pick(x):\n    alias = x\n    return alias",
    );
    let before_alias = host.resolve_name("b", "pick-alias", "chosen", "arg:literal:1");
    let consumer_before_alias = host.observable_resolution("b", "chosen", "arg:literal:1");
    let alias_consumer_count = host
        .body_counts()
        .get("observable:b:chosen:arg:literal:1")
        .copied()
        .unwrap_or(0);
    replace(
        &mut host,
        &mut current,
        "a.py",
        a,
        "equal summary local alias",
    )?;
    let after_alias = host.resolve_name("b", "pick-alias", "chosen", "arg:literal:1");
    let consumer_after_alias = host.observable_resolution("b", "chosen", "arg:literal:1");
    check(
        before_alias.targets == after_alias.targets
            && before_alias.behavior == after_alias.behavior,
        "local alias rewrite changed observable summary",
    )?;
    check(
        consumer_before_alias == consumer_after_alias,
        "equal summary alias edit changed consumer value",
    )?;
    check(
        host.body_counts()
            .get("observable:b:chosen:arg:literal:1")
            .copied()
            .unwrap_or(0)
            == alias_consumer_count,
        "equal summary body edit recomputed consumer",
    )?;
    publish(&mut host)?;
    edits += 1;

    // Change the selected function summary. Its consumer must execute and observe the new value.
    let a = current
        .iter()
        .find(|r| r.0 == "a.py")
        .unwrap()
        .2
        .replace("alias = x\n    return alias", "return 42");
    let resolver_before_change = host
        .body_counts()
        .get("resolve:b:observable:chosen:arg:literal:1")
        .copied()
        .unwrap_or(0);
    let changed = replace(
        &mut host,
        &mut current,
        "a.py",
        a,
        "change selected summary",
    )?;
    let consumer_changed = host.observable_resolution("b", "chosen", "arg:literal:1");
    check(
        changed.resolutions[0].targets == initial.resolutions[0].targets,
        "body change changed stable target identity",
    )?;
    check(
        changed.resolutions[0].behavior
            == vec![stack_graphs_incremental_prototype::resolver::Value::Literal("42".into())],
        "changed function summary did not update the call behavior",
    )?;
    check(
        consumer_changed != consumer_before_alias,
        "changed summary did not change the demanded consumer value",
    )?;
    let consumer_after = host
        .body_counts()
        .get("observable:b:chosen:arg:literal:1")
        .copied()
        .unwrap_or(0);
    let resolver_after_change = host
        .body_counts()
        .get("resolve:b:observable:chosen:arg:literal:1")
        .copied()
        .unwrap_or(0);
    check(resolver_after_change > resolver_before_change, &format!("changed target summary did not invalidate its consumer: before={resolver_before_change}, after={resolver_after_change}, observable={consumer_count}->{consumer_after}"))?;
    check(
        host.body_counts()
            .get("module:unrelated")
            .copied()
            .unwrap_or(0)
            == unrelated_initial,
        "unrelated module summary recomputed",
    )?;
    publish(&mut host)?;
    edits += 1;

    // Source locations change while semantic identities remain stable.
    let before_identity = changed.resolutions[0].targets.clone();
    let before_range = changed.projections["a.py"].as_ref().unwrap().ranges.clone();
    let a = format!(
        "marker = 0\n{}",
        current.iter().find(|r| r.0 == "a.py").unwrap().2
    );
    let shifted = replace(&mut host, &mut current, "a.py", a, "range shift")?;
    check(
        shifted.resolutions[0].targets == before_identity,
        "range-only edit changed semantic identity",
    )?;
    check(
        shifted.projections["a.py"].as_ref().unwrap().ranges != before_range,
        "source shift did not update current ranges",
    )?;
    publish(&mut host)?;
    edits += 1;

    // Valid source -> broken -> repaired produces explicit incomplete then complete results.
    let a = current
        .iter()
        .find(|r| r.0 == "a.py")
        .unwrap()
        .2
        .replace("def pick(x):\n    return 42", "def pick(x)\n    return 42");
    let broken = replace(&mut host, &mut current, "a.py", a, "break source")?;
    check(
        !broken.summaries["a"].complete && !broken.resolutions[0].complete,
        "broken source did not produce an incomplete outcome",
    )?;
    publish(&mut host)?;
    edits += 1;
    let a = current
        .iter()
        .find(|r| r.0 == "a.py")
        .unwrap()
        .2
        .replace("def pick(x)\n    return 42", "def pick(x):\n    return 7");
    let repaired = replace(&mut host, &mut current, "a.py", a, "repair source")?;
    check(
        repaired.summaries["a"].complete && repaired.resolutions[0].complete,
        &format!(
            "repaired source remained incomplete: summary={}, diagnostics={:?}, resolution={}",
            repaired.summaries["a"].complete,
            repaired.projections["a.py"].as_ref().unwrap().diagnostics,
            repaired.resolutions[0].complete
        ),
    )?;
    publish(&mut host)?;
    edits += 1;

    let seeded_cycle = host.resolve_name("a", "cycle-seed", "cycle_a", "arg:literal:true");
    check(
        seeded_cycle.behavior.contains(
            &stack_graphs_incremental_prototype::resolver::Value::Literal("'seed'".into()),
        ),
        &format!("source recursive SCC did not propagate its seed: {seeded_cycle:?}"),
    )?;
    let a = current
        .iter()
        .find(|r| r.0 == "a.py")
        .unwrap()
        .2
        .replace("return 'seed' if flag else cycle_b()", "return cycle_b()");
    let _ = replace(
        &mut host,
        &mut current,
        "a.py",
        a,
        "remove recursive source seed",
    )?;
    let unseeded_cycle = host.resolve_name("a", "cycle-seed", "cycle_a", "arg:literal:true");
    check(
        !unseeded_cycle.behavior.contains(
            &stack_graphs_incremental_prototype::resolver::Value::Literal("'seed'".into()),
        ),
        "deleted recursive source support remained in the fixed point",
    )?;
    publish(&mut host)?;
    edits += 1;

    // Recursive support and an external seed converge, then retract when the seed is removed.
    use stack_graphs_incremental_prototype::resolver::{solve, Formula, FunctionSummary};
    let seeded = vec![
        (
            "f".to_string(),
            FunctionSummary {
                params: 0,
                body: Formula::Choice(vec![
                    Formula::Literal("seed".into()),
                    Formula::Call("g".into(), vec![]),
                ]),
            },
        ),
        (
            "g".to_string(),
            FunctionSummary {
                params: 0,
                body: Formula::Call("f".into(), vec![]),
            },
        ),
    ]
    .into_iter()
    .collect();
    let solved = solve(&seeded, 16);
    check(
        solved["g"]
            .values
            .contains(&stack_graphs_incremental_prototype::resolver::Value::Literal("seed".into())),
        "recursive SCC did not propagate external seed",
    )?;
    let unseeded = vec![
        (
            "f".to_string(),
            FunctionSummary {
                params: 0,
                body: Formula::Call("g".into(), vec![]),
            },
        ),
        (
            "g".to_string(),
            FunctionSummary {
                params: 0,
                body: Formula::Call("f".into(), vec![]),
            },
        ),
    ]
    .into_iter()
    .collect();
    let after_delete = solve(&unseeded, 16);
    check(
        !after_delete["g"]
            .values
            .contains(&stack_graphs_incremental_prototype::resolver::Value::Literal("seed".into())),
        "deleted recursive support remained in the fixed point",
    )?;
    let _ = replace(
        &mut host,
        &mut current,
        "a.py",
        "def pick(x):\n    return x\n".into(),
        "recursive source reset",
    )?;
    publish(&mut host)?;
    edits += 1;

    // Removing the preferred target exposes the lower-precedence imported target.
    let prior = host.resolve_name("b", "target-delete", "chosen", "arg:literal:1");
    let removed = delete_document(&mut host, &mut current, "a.py", "delete preferred target")?;
    check(
        removed.resolutions[0].outcome == "unique"
            && removed.resolutions[0].targets != prior.targets,
        "deleting preferred target did not expose fallback",
    )?;
    publish(&mut host)?;
    edits += 1;
    let _ = add_document(
        &mut host,
        &mut current,
        "a.py",
        "a",
        A.into(),
        "restore preferred target",
    )?;
    publish(&mut host)?;
    edits += 1;

    // Local definitions shadow imports. Removing the local binding restores import precedence.
    let b = format!(
        "chosen = 10\n{}",
        current.iter().find(|r| r.0 == "b.py").unwrap().2
    );
    let shadow = replace(&mut host, &mut current, "b.py", b, "local shadow")?;
    let local_id = shadow.summaries["b"]
        .definitions
        .iter()
        .find(|d| d.name == "chosen")
        .map(|d| d.id.clone())
        .ok_or_else(|| "local shadow definition missing".to_string())?;
    check(
        shadow.resolutions[0].targets == vec![local_id],
        "local definition did not shadow imported name",
    )?;
    let b = current
        .iter()
        .find(|r| r.0 == "b.py")
        .unwrap()
        .2
        .replacen("chosen = 10\n", "", 1);
    let _ = replace(&mut host, &mut current, "b.py", b, "remove local shadow")?;
    publish(&mut host)?;
    edits += 2;

    // A missing module owns a stable empty bucket. Adding its document updates the lookup.
    check(
        host.resolve_name("b", "later-added", "later", "plain")
            .outcome
            == "missing",
        "missing module lookup was not tracked",
    )?;
    let _ = add_document(
        &mut host,
        &mut current,
        "ghost.py",
        "ghost",
        String::new(),
        "add empty dependency document",
    )?;
    check(
        host.resolve_name("b", "later-empty", "later", "plain")
            .outcome
            == "missing",
        "empty dependency document changed missing lookup",
    )?;
    let _ = replace(
        &mut host,
        &mut current,
        "ghost.py",
        "def later():\n    return 5\n".into(),
        "fill formerly empty module",
    )?;
    check(
        host.resolve_name("b", "later-added", "later", "plain")
            .outcome
            == "unique",
        "adding missing module did not update its dependent lookup",
    )?;
    publish(&mut host)?;
    edits += 1;

    // A fixed-seed edit stream adds and removes exports. Compare with a new host after each edit.
    let mut seed = 0x51a7_u32;
    let mut prior_random: Option<String> = None;
    for index in 0..8 {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let name = prior_random
            .take()
            .unwrap_or_else(|| format!("random_{}", seed % 97));
        let old = current.iter().find(|r| r.0 == "a.py").unwrap().2.clone();
        let adding = index % 2 == 0;
        let next = if adding {
            prior_random = Some(name.clone());
            format!("{old}\ndef {name}():\n    return {index}\n")
        } else {
            old.replace(
                &format!("\ndef {name}():\n    return {}\n", index - 1),
                "\n",
            )
        };
        let _ = replace(&mut host, &mut current, "a.py", next, "fixed-seed edit")?;
        publish(&mut host)?;
        edits += 1;
    }

    // No-op input and publication do not advance revision or write SQLite rows.
    let before_rev = host.revision();
    let before_audit = host.write_audit().map_err(|e| e.to_string())?.len();
    let a = current.iter().find(|r| r.0 == "a.py").unwrap();
    host.set_document(a.0, a.1, &a.2);
    check(
        host.revision() == before_rev,
        "identical input advanced the revision",
    )?;
    publish(&mut host)?;
    check(
        host.write_audit().map_err(|e| e.to_string())?.len() == before_audit,
        "no-op publication wrote SQLite rows",
    )?;
    check(
        host.apply_editor_overlay("a.py", "x = 1")
            == stack_graphs_incremental_prototype::OverlayResponse::UnsupportedSeparateOverlay,
        "overlay support contract changed",
    )?;
    check(
        host.revision_lifetime()
            == stack_graphs_incremental_prototype::RevisionLifetime::CurrentRevisionOnly,
        "revision lifetime contract missing",
    )?;

    // Shared support remains until its final owner is removed.
    let shared = PublishedFact {
        subject: "shared".into(),
        predicate: "tag".into(),
        object: "kept".into(),
        support: "module:a".into(),
    };
    let second = PublishedFact {
        support: "module:b".into(),
        ..shared.clone()
    };
    host.publish_supports(
        host.revision(),
        &[],
        &[shared.clone(), second.clone()],
        false,
        None,
    )?;
    host.publish_supports(host.revision(), &["module:a".into()], &[], false, None)?;
    check(
        host.published_facts()
            .map_err(|e| e.to_string())?
            .contains(&second),
        "removing one support deleted a shared fact",
    )?;

    // Obsolete, cancelled, and failed work preserves the previous committed publication.
    let committed = host.published_facts().map_err(|e| e.to_string())?;
    check(
        host.publish_supports(host.revision().saturating_sub(1), &[], &[], false, None)
            .is_err(),
        "stale publication was accepted",
    )?;
    check(
        host.publish_supports(host.revision(), &[], &[], true, None)
            .is_err(),
        "cancelled publication was accepted",
    )?;
    check(
        host.publish_supports(
            host.revision(),
            &[],
            &[PublishedFact {
                subject: "fail".into(),
                predicate: "p".into(),
                object: "o".into(),
                support: "module:a".into(),
            }],
            false,
            Some(0),
        )
        .is_err(),
        "injected publication failure was accepted",
    )?;
    check(
        host.published_facts().map_err(|e| e.to_string())? == committed,
        "failed transaction changed the previous publication",
    )?;
    let guard = host.publication_revision_guard();
    let obsolete_revision = host.revision();
    check(
        host.publish_supports_with_hook(
            obsolete_revision,
            &[],
            &[PublishedFact {
                subject: "obsolete".into(),
                predicate: "p".into(),
                object: "o".into(),
                support: "module:a".into(),
            }],
            false,
            None,
            || guard.store(obsolete_revision + 1, std::sync::atomic::Ordering::SeqCst),
        )
        .is_err(),
        "revision change at transaction boundary was accepted",
    )?;
    check(
        host.published_facts().map_err(|e| e.to_string())? == committed,
        "obsolete transaction changed the previous publication",
    )?;
    check(
        !host.write_audit().map_err(|e| e.to_string())?.is_empty(),
        "SQLite write audit did not record mutations",
    )?;
    check(
        host.body_counts()
            .get("module:unrelated")
            .copied()
            .unwrap_or(0)
            == unrelated_initial,
        "unrelated semantic module was not reused",
    )?;

    Ok(Report {
        status: "passed".into(),
        edits,
        expected: vec![
            "lowerer-derived argument and receiver contexts".into(),
            "successful, missing, and competing lookup outcomes".into(),
            "same answer retains cached consumer".into(),
            "changed summary invalidates affected consumer".into(),
            "recursive external support deletion".into(),
            "fresh-host differential after every edit".into(),
            "document roots discovered before publication".into(),
            "support-aware atomic SQLite publication".into(),
            "stale/cancel/failure rollback".into(),
            "no-op revisions and writes".into(),
            "fixed-seed edit stream".into(),
            "unrelated module reuse".into(),
        ],
        salsa_event_executions: host.execution_counts(),
        query_body_executions: host.body_counts(),
        memo_validations: host.validation_counts(),
        final_facts: host.published_facts().map_err(|e| e.to_string())?,
    })
}

fn main() {
    let state = PathBuf::from(std::env::var("STACK_GRAPHS_STATE").unwrap_or_else(|_| {
        format!(
            "{}/.local/state/stack-graphs/001-incremental-update",
            std::env::var("HOME").unwrap_or_else(|_| "/tmp".into())
        )
    }));
    if let Err(error) = fs::create_dir_all(&state) {
        eprintln!("cannot create state directory: {error}");
        std::process::exit(2);
    }
    match run() {
        Ok(report) => {
            let json = serde_json::to_string_pretty(&report).unwrap();
            fs::write(state.join("acceptance-report.json"), &json).expect("write report");
            println!("{json}");
        }
        Err(error) => {
            eprintln!("acceptance failed: {error}");
            std::process::exit(1);
        }
    }
}
