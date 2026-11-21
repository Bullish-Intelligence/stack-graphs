//! Experimental revisioned semantic analysis prototype.
pub mod resolver;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use salsa::Setter;
use serde::{Deserialize, Serialize};
use tree_sitter_stack_graphs_python::incremental::{
    lower_incremental_document, Completeness, Expression, SemanticKind,
};

#[salsa::db]
pub trait Db: salsa::Database {
    fn record_body(&self, key: String);
}

#[salsa::input]
pub struct Document {
    module: String,
    path: String,
    source: String,
}

#[salsa::input]
pub struct ModuleBucket {
    module: String,
    documents: Vec<Document>,
    dependencies: Vec<ModuleBucket>,
}

#[salsa::db]
#[derive(Clone)]
pub struct HostDb {
    storage: salsa::Storage<Self>,
    executions: Arc<Mutex<BTreeMap<String, usize>>>,
    validations: Arc<Mutex<BTreeMap<String, usize>>>,
    body_counts: Arc<Mutex<BTreeMap<String, usize>>>,
}

#[salsa::db]
impl Db for HostDb {
    fn record_body(&self, key: String) {
        *self.body_counts.lock().unwrap().entry(key).or_default() += 1;
    }
}

#[salsa::db]
impl salsa::Database for HostDb {}

impl Default for HostDb {
    fn default() -> Self {
        let executions = Arc::new(Mutex::new(BTreeMap::<String, usize>::new()));
        let validations = Arc::new(Mutex::new(BTreeMap::<String, usize>::new()));
        let body_counts = Arc::new(Mutex::new(BTreeMap::new()));
        let exec_events = executions.clone();
        let validate_events = validations.clone();
        let storage = salsa::Storage::builder()
            .event_callback(Box::new(move |event| {
                let (label, database_key, target) = match event.kind {
                    salsa::EventKind::WillExecute { database_key } => {
                        ("execute", database_key, &exec_events)
                    }
                    salsa::EventKind::DidValidateMemoizedValue { database_key } => {
                        ("validate", database_key, &validate_events)
                    }
                    _ => return,
                };
                let key = format!("{label}:{database_key:?}");
                *target.lock().unwrap().entry(key).or_default() += 1;
            }))
            .build();
        Self {
            storage,
            executions,
            validations,
            body_counts,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Definition {
    pub id: String,
    pub name: String,
    pub module: String,
    pub owner: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Reference {
    pub id: String,
    pub name: String,
    pub module: String,
    pub context: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub definitions: Vec<Definition>,
    pub references: Vec<Reference>,
    pub imports: Vec<Import>,
    pub functions: Vec<FunctionBehavior>,
    pub complete: bool,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Import {
    pub module: String,
    pub symbol: Option<String>,
    pub alias: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FunctionBehavior {
    pub id: String,
    pub name: String,
    pub summary: resolver::FunctionSummary,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct LoweringOutput {
    summary: Summary,
    ranges: Vec<(String, usize, usize, usize, usize, usize, usize)>,
    diagnostics: Vec<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Resolution {
    pub reference: Reference,
    pub targets: Vec<String>,
    pub candidates: Vec<String>,
    pub complete: bool,
    pub outcome: String,
    pub behavior: Vec<resolver::Value>,
}

fn encode<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("prototype value serializes")
}
fn decode<T: for<'de> Deserialize<'de>>(value: &str) -> T {
    serde_json::from_str(value).expect("prototype value decodes")
}

#[salsa::tracked(returns(clone))]
fn lowered(db: &dyn Db, document: Document) -> String {
    db.record_body(format!("lower:{}", document.path(db)));
    let module = document.module(db);
    let source = document.source(db);
    let path = document.path(db);
    let lowered = lower_incremental_document(module, path, source);
    let mut definitions = Vec::new();
    let mut references = Vec::new();
    let mut imports = Vec::new();
    let mut complete = matches!(lowered.completeness, Completeness::Complete);
    let owner_names: BTreeMap<String, String> = lowered
        .fragment
        .items
        .iter()
        .filter(|i| i.kind == SemanticKind::Class)
        .filter_map(|i| i.name.as_ref().map(|n| (i.id.clone(), n.clone())))
        .collect();
    let class_ids: BTreeSet<String> = owner_names.keys().cloned().collect();
    for item in &lowered.fragment.items {
        if let SemanticKind::Import {
            module: target_module,
            symbol,
            alias,
        } = &item.kind
        {
            imports.push(Import {
                module: target_module.clone(),
                symbol: symbol.clone(),
                alias: alias.clone(),
            });
        }
        match &item.kind {
            SemanticKind::Function | SemanticKind::Class => {
                if item.parent_id.is_none()
                    || item
                        .parent_id
                        .as_ref()
                        .map(|id| class_ids.contains(id))
                        .unwrap_or(false)
                {
                    if let Some(name) = &item.name {
                        definitions.push(Definition {
                            id: item.id.clone(),
                            name: name.clone(),
                            module: module.to_string(),
                            owner: item
                                .parent_id
                                .as_ref()
                                .and_then(|id| owner_names.get(id).cloned()),
                        });
                    }
                }
            }
            SemanticKind::Assignment => {
                if item.parent_id.is_none()
                    || item
                        .parent_id
                        .as_ref()
                        .map(|id| class_ids.contains(id))
                        .unwrap_or(false)
                {
                    if let Some(name) = &item.name {
                        definitions.push(Definition {
                            id: item.id.clone(),
                            name: name.clone(),
                            module: module.to_string(),
                            owner: item
                                .parent_id
                                .as_ref()
                                .and_then(|id| owner_names.get(id).cloned()),
                        });
                    }
                }
            }
            SemanticKind::Call => {
                if let Some(Expression::Call { callee, .. }) = &item.expression {
                    if let Some(name) = expression_name(callee) {
                        references.push(Reference {
                            id: item.id.clone(),
                            name,
                            module: module.to_string(),
                            context: expression_context(item.expression.as_ref()),
                        });
                    }
                }
            }
            SemanticKind::Attribute => {
                if !matches!(item.expression.as_ref(), Some(Expression::Attribute { receiver, .. }) if receiver_class(receiver).is_some())
                {
                    complete = false;
                }
            }
            _ => {}
        }
    }
    let fn_ids: BTreeMap<_, _> = lowered
        .fragment
        .items
        .iter()
        .filter(|i| i.kind == SemanticKind::Function)
        .filter_map(|i| i.name.as_ref().map(|n| (n.clone(), i.id.clone())))
        .collect();
    let item_by_id: BTreeMap<_, _> = lowered
        .fragment
        .items
        .iter()
        .map(|i| (i.id.as_str(), i))
        .collect();
    let mut functions = Vec::new();
    for item in &lowered.fragment.items {
        if item.kind == SemanticKind::Function {
            let mut env: BTreeMap<String, resolver::Formula> = item
                .params
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    (
                        p.clone(),
                        if p == "self" {
                            resolver::Formula::Receiver
                        } else {
                            resolver::Formula::Argument(i)
                        },
                    )
                })
                .collect();
            let owned: Vec<_> = lowered
                .fragment
                .items
                .iter()
                .filter(|child| is_within_function(child, &item.id, &item_by_id))
                .collect();
            let mut returns = Vec::new();
            for child in owned {
                if child.kind == SemanticKind::Assignment {
                    if let (Some(name), Some(expr)) = (&child.name, &child.expression) {
                        env.insert(name.clone(), to_formula(expr, &env, &fn_ids));
                    }
                } else if child.kind == SemanticKind::Return {
                    if let Some(expr) = &child.expression {
                        returns.push(to_formula(expr, &env, &fn_ids));
                    }
                }
            }
            let formula = if returns.len() == 1 {
                returns.remove(0)
            } else if returns.is_empty() {
                resolver::Formula::Unknown
            } else {
                resolver::Formula::Choice(returns)
            };
            complete &= formula_supported(&formula);
            functions.push(FunctionBehavior {
                id: item.id.clone(),
                name: item.name.clone().unwrap_or_default(),
                summary: resolver::FunctionSummary {
                    params: item.params.len(),
                    body: formula,
                },
            });
        }
    }
    let ranges = lowered
        .source_map
        .entries
        .iter()
        .map(|r| {
            (
                r.item_id.clone(),
                r.start_byte,
                r.end_byte,
                r.start_row,
                r.start_column,
                r.end_row,
                r.end_column,
            )
        })
        .collect();
    let diagnostics = lowered
        .diagnostics
        .iter()
        .map(|d| d.message.clone())
        .collect();
    encode(&LoweringOutput {
        summary: Summary {
            definitions,
            references,
            imports,
            functions,
            complete,
        },
        ranges,
        diagnostics,
    })
}

fn is_within_function<'a>(
    item: &'a tree_sitter_stack_graphs_python::incremental::SemanticItem,
    function_id: &str,
    items: &BTreeMap<&str, &'a tree_sitter_stack_graphs_python::incremental::SemanticItem>,
) -> bool {
    let mut parent = item.parent_id.as_deref();
    while let Some(id) = parent {
        if id == function_id {
            return true;
        }
        let Some(parent_item) = items.get(id) else {
            return false;
        };
        if parent_item.kind == SemanticKind::Function || parent_item.kind == SemanticKind::Class {
            return false;
        }
        parent = parent_item.parent_id.as_deref();
    }
    false
}

fn formula_supported(formula: &resolver::Formula) -> bool {
    match formula {
        resolver::Formula::Unknown => false,
        resolver::Formula::Call(name, args) => {
            name != "<dynamic>" && args.iter().all(formula_supported)
        }
        resolver::Formula::Choice(items) => items.iter().all(formula_supported),
        _ => true,
    }
}

#[salsa::tracked(returns(clone))]
fn semantic_projection(db: &dyn Db, document: Document) -> String {
    encode(&decode::<LoweringOutput>(&lowered(db, document)).summary)
}

#[salsa::tracked(returns(clone))]
fn source_map_projection(db: &dyn Db, document: Document) -> String {
    let output = decode::<LoweringOutput>(&lowered(db, document));
    encode(&(output.ranges, output.diagnostics))
}

fn to_formula(
    expression: &Expression,
    env: &BTreeMap<String, resolver::Formula>,
    fn_ids: &BTreeMap<String, String>,
) -> resolver::Formula {
    match expression {
        Expression::Name(name) => env.get(name).cloned().unwrap_or(resolver::Formula::Unknown),
        Expression::Literal(value) => resolver::Formula::Literal(value.clone()),
        Expression::Call { callee, args } => {
            let name = match callee.as_ref() {
                Expression::Name(n) => n.clone(),
                Expression::Attribute { name, .. } => name.clone(),
                _ => "<dynamic>".into(),
            };
            resolver::Formula::Call(
                fn_ids.get(&name).cloned().unwrap_or(name),
                args.iter().map(|a| to_formula(a, env, fn_ids)).collect(),
            )
        }
        Expression::Attribute { receiver, name } => match to_formula(receiver, env, fn_ids) {
            resolver::Formula::Receiver => resolver::Formula::Member(name.clone()),
            _ => resolver::Formula::Unknown,
        },
        Expression::Conditional { branches } | Expression::Tuple(branches) => {
            resolver::Formula::Choice(
                branches
                    .iter()
                    .map(|b| to_formula(b, env, fn_ids))
                    .collect(),
            )
        }
        Expression::Unknown => resolver::Formula::Unknown,
    }
}

fn expression_context(expr: Option<&Expression>) -> String {
    match expr {
        Some(Expression::Call { callee, args }) => {
            if let Expression::Attribute { receiver, .. } = callee.as_ref() {
                if let Some(class) = receiver_class(receiver) {
                    return format!("receiver:class:{class}");
                }
            }
            args.first()
                .map(|a| format!("arg:{}", expression_key(a)))
                .unwrap_or_else(|| "arg:none".into())
        }
        Some(Expression::Attribute { receiver, .. }) => {
            format!("receiver:{}", expression_key(receiver))
        }
        _ => "plain".into(),
    }
}
fn receiver_class(expr: &Expression) -> Option<&str> {
    match expr {
        Expression::Call { callee, .. } => match callee.as_ref() {
            Expression::Name(name) => Some(name),
            _ => None,
        },
        _ => None,
    }
}
fn expression_name(expr: &Expression) -> Option<String> {
    match expr {
        Expression::Name(name) => Some(name.clone()),
        Expression::Attribute { name, .. } => Some(name.clone()),
        _ => None,
    }
}
fn expression_key(expr: &Expression) -> String {
    match expr {
        Expression::Name(n) => format!("name:{n}"),
        Expression::Literal(v) => format!("literal:{v}"),
        Expression::Call { callee, args } => format!(
            "call:{}({})",
            expression_key(callee),
            args.iter()
                .map(expression_key)
                .collect::<Vec<_>>()
                .join(",")
        ),
        Expression::Attribute { receiver, name } => {
            format!("attribute:{}.{}", expression_key(receiver), name)
        }
        Expression::Tuple(xs) => format!(
            "tuple:{}",
            xs.iter().map(expression_key).collect::<Vec<_>>().join(",")
        ),
        Expression::Conditional { branches } => format!(
            "choice:{}",
            branches
                .iter()
                .map(expression_key)
                .collect::<Vec<_>>()
                .join("|")
        ),
        Expression::Unknown => "unknown".into(),
    }
}

#[salsa::tracked(returns(clone))]
fn module_summary(db: &dyn Db, bucket: ModuleBucket) -> String {
    let module = bucket.module(db).clone();
    db.record_body(format!("module:{module}"));
    let mut summaries = Vec::new();
    for doc in bucket.documents(db) {
        summaries.push(decode::<Summary>(&semantic_projection(db, *doc)));
    }
    let mut merged = Summary {
        definitions: Vec::new(),
        references: Vec::new(),
        imports: Vec::new(),
        functions: Vec::new(),
        complete: true,
    };
    for summary in summaries {
        merged.complete &= summary.complete;
        merged.definitions.extend(summary.definitions);
        merged.references.extend(summary.references);
        merged.imports.extend(summary.imports);
        merged.functions.extend(summary.functions);
    }
    merged.definitions.sort_by(|a, b| a.id.cmp(&b.id));
    merged.references.sort_by(|a, b| a.id.cmp(&b.id));
    encode(&merged)
}

#[salsa::tracked(returns(clone))]
fn resolve(
    db: &dyn Db,
    bucket: ModuleBucket,
    reference_id: String,
    name: String,
    context: String,
) -> String {
    let module = bucket.module(db).clone();
    let dependencies = bucket.dependencies(db);
    db.record_body(format!("resolve:{module}:{reference_id}:{name}:{context}"));
    let summary = decode::<Summary>(&module_summary(db, bucket));
    let receiver_class = context.strip_prefix("receiver:class:");
    let mut candidates: Vec<_> = summary
        .definitions
        .iter()
        .filter(|d| {
            d.name == name
                && receiver_class
                    .map(|c| d.owner.as_deref() == Some(c))
                    .unwrap_or(true)
        })
        .map(|d| d.id.clone())
        .collect();
    let mut complete = summary.complete;
    // Local bindings have precedence. Imports are consulted only when no local binding exists.
    if candidates.is_empty() {
        let mut selected = false;
        for import in summary.imports.iter().filter(|i| {
            i.alias
                .as_deref()
                .unwrap_or(i.symbol.as_deref().unwrap_or(""))
                == name
                || (i.symbol.is_none() && i.module.rsplit('.').next().unwrap_or("") == name)
        }) {
            let Some(target_bucket) = dependencies.iter().find(|b| b.module(db) == &import.module)
            else {
                continue;
            };
            let target = decode::<Summary>(&module_summary(db, *target_bucket));
            if !selected {
                complete &= target.complete;
            }
            let symbol = import.symbol.as_deref().unwrap_or(name.as_str());
            let imported: Vec<_> = target
                .definitions
                .iter()
                .filter(|d| d.name == symbol)
                .map(|d| d.id.clone())
                .collect();
            if !selected && !imported.is_empty() {
                candidates = imported;
                selected = true;
            }
        }
    }
    if candidates.is_empty() {
        if let Some(class) = receiver_class {
            for target_bucket in dependencies {
                let target = decode::<Summary>(&module_summary(db, *target_bucket));
                let members: Vec<_> = target
                    .definitions
                    .iter()
                    .filter(|d| d.name == name && d.owner.as_deref() == Some(class))
                    .map(|d| d.id.clone())
                    .collect();
                if !members.is_empty() {
                    candidates = members;
                    complete &= target.complete;
                    break;
                }
            }
        }
    }
    // An explicit import context records a competing or missing module lookup.
    if candidates.is_empty() && context.starts_with("module:") {
        let target_module = context.trim_start_matches("module:");
        if let Some(target_bucket) = dependencies.iter().find(|b| b.module(db) == target_module) {
            let target = decode::<Summary>(&module_summary(db, *target_bucket));
            complete &= target.complete;
            candidates.extend(
                target
                    .definitions
                    .iter()
                    .filter(|d| d.name == name)
                    .map(|d| d.id.clone()),
            );
        }
    }
    candidates.sort();
    candidates.dedup();
    let targets = if candidates.len() == 1 {
        candidates.clone()
    } else {
        Vec::new()
    };
    let outcome = match candidates.len() {
        0 => "missing",
        1 => "unique",
        _ => "competing",
    }
    .to_string();
    let mut behavior = Vec::new();
    if let Some(target_id) = targets.first() {
        for candidate_module in std::iter::once(module.as_str())
            .chain(summary.imports.iter().map(|i| i.module.as_str()))
        {
            let target = if candidate_module == module {
                summary.clone()
            } else {
                let Some(target_bucket) = dependencies
                    .iter()
                    .find(|b| b.module(db) == candidate_module)
                else {
                    continue;
                };
                decode::<Summary>(&module_summary(db, *target_bucket))
            };
            if let Some(function) = target.functions.iter().find(|f| &f.id == target_id) {
                let args = context
                    .strip_prefix("arg:")
                    .map(|s| vec![parse_abstract_value(s)])
                    .unwrap_or_default();
                let receiver = context.strip_prefix("receiver:").map(parse_abstract_value);
                let functions: BTreeMap<_, _> = target
                    .functions
                    .iter()
                    .map(|f| (f.id.clone(), f.summary.clone()))
                    .collect();
                let solved = resolver::solve(&functions, 32);
                complete &= formula_supported(&function.summary.body);
                behavior.extend(resolver::apply(
                    &function.summary.body,
                    &args,
                    receiver.as_ref(),
                    &solved,
                ));
                break;
            }
        }
    }
    encode(&Resolution {
        reference: Reference {
            id: reference_id,
            name,
            module,
            context,
        },
        targets,
        candidates,
        complete,
        outcome,
        behavior,
    })
}

#[salsa::tracked(returns(clone))]
fn observable_resolution(
    db: &dyn Db,
    bucket: ModuleBucket,
    name: String,
    context: String,
) -> String {
    let module = bucket.module(db).clone();
    db.record_body(format!("observable:{module}:{name}:{context}"));
    let result = decode::<Resolution>(&resolve(db, bucket, "observable".into(), name, context));
    encode(&(
        result.targets,
        result.candidates,
        result.complete,
        result.outcome,
        result.behavior,
    ))
}

fn parse_abstract_value(text: &str) -> resolver::Value {
    if let Some(value) = text.strip_prefix("literal:") {
        resolver::Value::Literal(value.to_string())
    } else if text == "unknown" {
        resolver::Value::Unknown
    } else {
        resolver::Value::Symbolic(text.to_string())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct PublishedFact {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub support: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DocumentProjection {
    pub facts: Summary,
    pub ranges: Vec<(String, usize, usize, usize, usize, usize, usize)>,
    pub diagnostics: Vec<String>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum OverlayResponse {
    UnsupportedSeparateOverlay,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RevisionLifetime {
    CurrentRevisionOnly,
}

pub struct AnalysisHost {
    pub db: HostDb,
    buckets: BTreeMap<String, ModuleBucket>,
    documents: BTreeMap<String, Document>,
    revision: u64,
    revision_guard: Arc<AtomicU64>,
    pub publication: Connection,
}
impl AnalysisHost {
    pub fn new() -> rusqlite::Result<Self> {
        let db = HostDb::default();
        let publication = Connection::open_in_memory()?;
        publication.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE IF NOT EXISTS facts(subject TEXT NOT NULL,predicate TEXT NOT NULL,object TEXT NOT NULL,support TEXT NOT NULL,PRIMARY KEY(subject,predicate,object,support)); CREATE TABLE IF NOT EXISTS publications(revision INTEGER PRIMARY KEY, complete INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS write_audit(action TEXT NOT NULL, subject TEXT NOT NULL); CREATE TRIGGER IF NOT EXISTS audit_insert AFTER INSERT ON facts BEGIN INSERT INTO write_audit VALUES ('insert_fact',NEW.subject); END; CREATE TRIGGER IF NOT EXISTS audit_delete AFTER DELETE ON facts BEGIN INSERT INTO write_audit VALUES ('delete_fact',OLD.subject); END; CREATE TRIGGER IF NOT EXISTS audit_publication_insert AFTER INSERT ON publications BEGIN INSERT INTO write_audit VALUES ('insert_publication',CAST(NEW.revision AS TEXT)); END; CREATE TRIGGER IF NOT EXISTS audit_publication_update AFTER UPDATE ON publications BEGIN INSERT INTO write_audit VALUES ('update_publication',CAST(NEW.revision AS TEXT)); END; CREATE TRIGGER IF NOT EXISTS audit_publication_delete AFTER DELETE ON publications BEGIN INSERT INTO write_audit VALUES ('delete_publication',CAST(OLD.revision AS TEXT)); END;")?;
        Ok(Self {
            db,
            buckets: BTreeMap::new(),
            documents: BTreeMap::new(),
            revision: 0,
            revision_guard: Arc::new(AtomicU64::new(0)),
            publication,
        })
    }
    pub fn set_document(&mut self, file: &str, module: &str, source: &str) {
        if let Some(existing) = self.documents.get(file) {
            if existing.module(&self.db) == module && existing.source(&self.db) == source {
                return;
            }
        }
        let old_module = self.documents.get(file).map(|d| d.module(&self.db).clone());
        let doc = if let Some(doc) = self.documents.get(file) {
            doc.set_module(&mut self.db).to(module.to_string());
            doc.set_source(&mut self.db).to(source.to_string());
            doc.clone()
        } else {
            Document::new(
                &self.db,
                module.to_string(),
                file.to_string(),
                source.to_string(),
            )
        };
        self.documents.insert(file.to_string(), doc);
        self.ensure_bucket(module);
        if let Some(old) = old_module {
            self.ensure_bucket(&old);
        }
        self.sync_buckets();
        self.revision += 1;
        self.revision_guard.store(self.revision, Ordering::SeqCst);
    }
    pub fn remove_document(&mut self, file: &str) {
        let Some(old) = self
            .documents
            .remove(file)
            .map(|d| d.module(&self.db).clone())
        else {
            return;
        };
        self.ensure_bucket(&old);
        self.sync_buckets();
        self.revision += 1;
        self.revision_guard.store(self.revision, Ordering::SeqCst);
    }
    fn ensure_bucket(&mut self, module: &str) -> ModuleBucket {
        if let Some(bucket) = self.buckets.get(module) {
            return *bucket;
        }
        let bucket = ModuleBucket::new(&self.db, module.to_string(), Vec::new(), Vec::new());
        self.buckets.insert(module.to_string(), bucket);
        bucket
    }
    fn sync_buckets(&mut self) {
        let mut dependencies_by_module: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for doc in self.documents.values().copied() {
            let module = doc.module(&self.db).clone();
            let output = decode::<LoweringOutput>(&lowered(&self.db, doc));
            dependencies_by_module
                .entry(module)
                .or_default()
                .extend(output.summary.imports.into_iter().map(|i| i.module));
        }
        let referenced: Vec<_> = dependencies_by_module
            .values()
            .flat_map(|names| names.iter().cloned())
            .collect();
        for module in referenced {
            self.ensure_bucket(&module);
        }
        let modules: Vec<_> = self.buckets.keys().cloned().collect();
        for module in modules {
            let docs: Vec<_> = self
                .documents
                .values()
                .copied()
                .filter(|d| d.module(&self.db) == &module)
                .collect();
            let bucket = self.buckets[&module];
            if bucket.documents(&self.db) != &docs {
                bucket.set_documents(&mut self.db).to(docs);
            }
            let dependencies: Vec<_> = dependencies_by_module
                .get(&module)
                .into_iter()
                .flatten()
                .filter_map(|name| self.buckets.get(name).copied())
                .collect();
            if bucket.dependencies(&self.db) != &dependencies {
                bucket.set_dependencies(&mut self.db).to(dependencies);
            }
        }
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn publication_revision_guard(&self) -> Arc<AtomicU64> {
        self.revision_guard.clone()
    }
    pub fn document_count(&self) -> usize {
        self.documents.len()
    }
    pub fn revision_lifetime(&self) -> RevisionLifetime {
        RevisionLifetime::CurrentRevisionOnly
    }
    pub fn apply_editor_overlay(&mut self, _file: &str, _source: &str) -> OverlayResponse {
        OverlayResponse::UnsupportedSeparateOverlay
    }
    pub fn summary(&self, module: &str) -> Summary {
        self.buckets
            .get(module)
            .map(|b| decode(&module_summary(&self.db, *b)))
            .unwrap_or(Summary {
                definitions: Vec::new(),
                references: Vec::new(),
                imports: Vec::new(),
                functions: Vec::new(),
                complete: true,
            })
    }
    /// Discover document facts and current ranges without requiring an earlier editor query.
    pub fn document_projection(&self, file: &str) -> Option<DocumentProjection> {
        let doc = self.documents.get(file)?;
        let facts = decode::<Summary>(&semantic_projection(&self.db, *doc));
        let (ranges, diagnostics) = decode::<(
            Vec<(String, usize, usize, usize, usize, usize, usize)>,
            Vec<String>,
        )>(&source_map_projection(&self.db, *doc));
        Some(DocumentProjection {
            facts,
            ranges,
            diagnostics,
        })
    }
    pub fn discover_all_documents(&self) -> BTreeMap<String, DocumentProjection> {
        self.documents
            .keys()
            .filter_map(|file| {
                self.document_projection(file)
                    .map(|facts| (file.clone(), facts))
            })
            .collect()
    }
    pub fn resolve_name(
        &mut self,
        module: &str,
        reference_id: &str,
        name: &str,
        context: &str,
    ) -> Resolution {
        let bucket = self.ensure_bucket(module);
        decode(&resolve(
            &self.db,
            bucket,
            reference_id.to_string(),
            name.to_string(),
            context.to_string(),
        ))
    }
    pub fn observable_resolution(&mut self, module: &str, name: &str, context: &str) -> String {
        let bucket = self.ensure_bucket(module);
        observable_resolution(&self.db, bucket, name.into(), context.into())
    }
    pub fn execution_counts(&self) -> BTreeMap<String, usize> {
        self.db.executions.lock().unwrap().clone()
    }
    pub fn validation_counts(&self) -> BTreeMap<String, usize> {
        self.db.validations.lock().unwrap().clone()
    }
    pub fn body_counts(&self) -> BTreeMap<String, usize> {
        self.db.body_counts.lock().unwrap().clone()
    }
    pub fn publish(&mut self, revision: u64, facts: &[PublishedFact]) -> Result<(), String> {
        self.publish_supports(revision, &[], facts, false, None)
    }
    /// Replace selected support owners in one transaction. Other supports retain shared facts.
    pub fn publish_supports(
        &mut self,
        revision: u64,
        replace_supports: &[String],
        facts: &[PublishedFact],
        cancelled: bool,
        fail_after: Option<usize>,
    ) -> Result<(), String> {
        self.publish_supports_with_hook(
            revision,
            replace_supports,
            facts,
            cancelled,
            fail_after,
            || {},
        )
    }
    pub fn publish_supports_with_hook<F: FnOnce()>(
        &mut self,
        revision: u64,
        replace_supports: &[String],
        facts: &[PublishedFact],
        cancelled: bool,
        fail_after: Option<usize>,
        before_commit: F,
    ) -> Result<(), String> {
        if revision != self.revision || revision != self.revision_guard.load(Ordering::SeqCst) {
            return Err("stale revision".into());
        }
        if cancelled {
            return Err("publication cancelled".into());
        }
        let conn = self
            .publication
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let replacing: BTreeSet<_> = replace_supports.iter().cloned().collect();
        let desired: BTreeSet<_> = facts.iter().cloned().collect();
        let existing: Vec<PublishedFact> = {
            let mut stmt = conn
                .prepare("SELECT subject,predicate,object,support FROM facts")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |row| {
                    Ok(PublishedFact {
                        subject: row.get(0)?,
                        predicate: row.get(1)?,
                        object: row.get(2)?,
                        support: row.get(3)?,
                    })
                })
                .map_err(|e| e.to_string())?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|e| e.to_string())?;
            rows
        };
        for old in &existing {
            if replacing.contains(&old.support) && !desired.contains(old) {
                conn.execute("DELETE FROM facts WHERE subject=?1 AND predicate=?2 AND object=?3 AND support=?4", (&old.subject,&old.predicate,&old.object,&old.support)).map_err(|e| e.to_string())?;
            }
        }
        for (index, fact) in facts.iter().enumerate() {
            if fail_after == Some(index) {
                return Err("injected publication failure".into());
            }
            if !existing.contains(fact) {
                conn.execute(
                    "INSERT INTO facts VALUES (?1,?2,?3,?4)",
                    (&fact.subject, &fact.predicate, &fact.object, &fact.support),
                )
                .map_err(|e| e.to_string())?;
            }
        }
        let published_revision: Option<i64> = conn
            .query_row(
                "SELECT revision FROM publications ORDER BY revision DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if published_revision != Some(revision as i64) {
            conn.execute(
                "INSERT OR REPLACE INTO publications VALUES (?1,1)",
                [revision],
            )
            .map_err(|e| e.to_string())?;
        }
        before_commit();
        if revision != self.revision
            || revision != self.revision_guard.load(Ordering::SeqCst)
            || cancelled
        {
            return Err("publication became obsolete".into());
        }
        conn.commit().map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn published_facts(&self) -> rusqlite::Result<Vec<PublishedFact>> {
        let mut statement = self.publication.prepare("SELECT subject,predicate,object,support FROM facts ORDER BY subject,predicate,object,support")?;
        let rows = statement
            .query_map([], |row| {
                Ok(PublishedFact {
                    subject: row.get(0)?,
                    predicate: row.get(1)?,
                    object: row.get(2)?,
                    support: row.get(3)?,
                })
            })?
            .collect();
        rows
    }
    pub fn write_audit(&self) -> rusqlite::Result<Vec<(String, String)>> {
        let mut statement = self
            .publication
            .prepare("SELECT action,subject FROM write_audit ORDER BY rowid")?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect();
        rows
    }
}

pub fn facts_for(host: &mut AnalysisHost, modules: &[&str]) -> Vec<PublishedFact> {
    let mut facts = Vec::new();
    for module in modules {
        let summary = host.summary(module);
        for def in summary.definitions {
            facts.push(PublishedFact {
                subject: def.id,
                predicate: "defines".into(),
                object: def.name,
                support: format!("module:{module}"),
            });
        }
        for import in &summary.imports {
            let name = import
                .alias
                .as_deref()
                .or(import.symbol.as_deref())
                .unwrap_or("");
            let lookup_name = import.symbol.as_deref().unwrap_or(name);
            let result = host.resolve_name(
                module,
                name,
                lookup_name,
                &format!("module:{}", import.module),
            );
            for target in result.targets {
                facts.push(PublishedFact {
                    subject: format!("{}:{name}", module),
                    predicate: "imports".into(),
                    object: target,
                    support: format!("module:{module}"),
                });
            }
        }
        for reference in summary.references {
            let result =
                host.resolve_name(module, &reference.id, &reference.name, &reference.context);
            for target in result.targets {
                facts.push(PublishedFact {
                    subject: reference.id.clone(),
                    predicate: "resolves_to".into(),
                    object: target,
                    support: format!("module:{module}"),
                });
            }
        }
    }
    facts.sort_by(|a, b| {
        (&a.subject, &a.predicate, &a.object, &a.support).cmp(&(
            &b.subject,
            &b.predicate,
            &b.object,
            &b.support,
        ))
    });
    facts
}
