// -*- coding: utf-8 -*-
// ------------------------------------------------------------------------------------------------
// Copyright © 2024, stack-graphs authors.
// Licensed under either of Apache License, Version 2.0, or MIT license, at your option.
// Please see the LICENSE-APACHE or LICENSE-MIT files in this distribution for license details.
// ------------------------------------------------------------------------------------------------

//! Tests that names in Python source resolve to definitions in `src/builtins.py`.
//!
//! The `.py` fixtures in `test/` cannot check this. The loader adds the builtins graph to every
//! fixture graph, but a `defined:` assertion can only name lines of the fixture file.
//! This test resolves the references itself and reads the line of each definition in the builtins.

use stack_graphs::graph::StackGraph;
use stack_graphs::partial::PartialPaths;
use stack_graphs::stitching::Database;
use stack_graphs::stitching::DatabaseCandidates;
use stack_graphs::stitching::ForwardPartialPathStitcher;
use stack_graphs::stitching::StitcherConfig;
use tree_sitter_stack_graphs::NoCancellation;
use tree_sitter_stack_graphs::Variables;
use tree_sitter_stack_graphs_python::STACK_GRAPHS_BUILTINS_SOURCE;

const BUILTINS_FILE: &str = "<builtins>";
const TEST_FILE: &str = "test.py";

/// One resolved definition of a reference.
#[derive(Debug, PartialEq, Eq)]
enum Target {
    /// A definition in the builtins. The string is the source line of the definition.
    Builtin(String),
    /// A definition in the test source. The number is the zero-based line.
    Local(usize),
}

/// Resolve all references in `source` and return the targets of the references named `name`.
fn resolve(source: &str, name: &str) -> Vec<Target> {
    let lc = tree_sitter_stack_graphs_python::language_configuration(&NoCancellation);
    let mut graph = StackGraph::new();
    graph.add_from_graph(&lc.builtins).expect("add builtins");
    let file = graph.get_or_create_file(TEST_FILE);
    let mut globals = Variables::new();
    globals
        .add("FILE_PATH".into(), TEST_FILE.into())
        .expect("set FILE_PATH");
    lc.sgl
        .build_stack_graph_into(&mut graph, file, source, &globals, &NoCancellation)
        .expect("build test source");

    let mut partials = PartialPaths::new();
    let mut db = Database::new();
    let files = graph.iter_files().collect::<Vec<_>>();
    for f in files {
        let mut paths = Vec::new();
        ForwardPartialPathStitcher::find_minimal_partial_path_set_in_file(
            &graph,
            &mut partials,
            f,
            StitcherConfig::default(),
            &stack_graphs::NoCancellation,
            |_, _, p| paths.push(p.clone()),
        )
        .expect("partial paths");
        for p in paths {
            db.add_partial_path(&graph, &mut partials, p);
        }
    }

    let references = graph
        .iter_nodes()
        .filter(|n| graph[*n].is_reference())
        .filter(|n| graph[*n].file() == Some(file))
        .filter(|n| graph[*n].symbol().map(|s| &graph[s] == name).unwrap_or(false))
        .collect::<Vec<_>>();
    assert!(!references.is_empty(), "no reference named {}", name);

    let builtin_lines = STACK_GRAPHS_BUILTINS_SOURCE.lines().collect::<Vec<_>>();
    let mut targets = Vec::new();
    for reference in references {
        let mut paths = Vec::new();
        ForwardPartialPathStitcher::find_all_complete_partial_paths(
            &mut DatabaseCandidates::new(&graph, &mut partials, &db),
            std::iter::once(reference),
            StitcherConfig::default(),
            &stack_graphs::NoCancellation,
            |_, _, p| paths.push(p.clone()),
        )
        .expect("resolve");
        // Drop the paths that another path shadows, as the test runner does.
        let ends = paths
            .iter()
            .filter(|p| paths.iter().all(|o| !o.shadows(&mut partials, p)))
            .map(|p| p.end_node)
            .collect::<Vec<_>>();
        for end in ends {
            let line = graph.source_info(end).expect("source info").span.start.line;
            let end_file = graph[end].file().expect("definition file");
            let target = if graph[end_file].name() == BUILTINS_FILE {
                Target::Builtin(builtin_lines[line].trim().to_string())
            } else {
                Target::Local(line)
            };
            if !targets.contains(&target) {
                targets.push(target);
            }
        }
    }
    targets
}

fn assert_builtin(source: &str, name: &str, definition: &str) {
    let targets = resolve(source, name);
    assert_eq!(
        targets,
        vec![Target::Builtin(definition.to_string())],
        "reference {} in:\n{}",
        name,
        source
    );
}

#[test]
fn builtin_function_len() {
    assert_builtin("x = len([1, 2])\n", "len", "def len(*args, **kwargs): ...");
}

#[test]
fn builtin_function_isinstance() {
    assert_builtin(
        "def f(x):\n    return isinstance(x, int)\n",
        "isinstance",
        "def isinstance(*args, **kwargs): ...",
    );
}

#[test]
fn builtin_class() {
    assert_builtin("x = int('1')\n", "int", "class int:");
    assert_builtin("x = super()\n", "super", "class super:");
}

#[test]
fn builtin_exception_class() {
    assert_builtin(
        "def f():\n    raise ValueError('bad')\n",
        "ValueError",
        "class ValueError(Exception):",
    );
}

#[test]
fn builtin_exception_in_except_clause() {
    assert_builtin(
        "try:\n    pass\nexcept KeyError as e:\n    pass\n",
        "KeyError",
        "class KeyError(LookupError):",
    );
}

#[test]
fn builtin_exception_base_class() {
    // The base class `Exception` resolves, because it is a builtin too.
    assert_builtin(
        "class MyError(Exception):\n    pass\n",
        "Exception",
        "class Exception(BaseException):",
    );
}

#[test]
fn builtin_class_method_through_class() {
    assert_builtin(
        "x = str.join(',', ['a'])\n",
        "join",
        "def join(self, *args, **kwargs): ...",
    );
}

#[test]
fn builtin_method_inherited_from_base_class() {
    // `with_traceback` is defined in `BaseException`, the base of `Exception`.
    assert_builtin(
        "x = Exception().with_traceback\n",
        "with_traceback",
        "def with_traceback(self, *args, **kwargs): ...",
    );
}

#[test]
fn builtin_member_through_user_subclass_of_exception() {
    // `MyError` derives from `Exception`, which derives from `BaseException`.
    let source = "class MyError(Exception):\n    pass\n\nx = MyError().args\ny = MyError().with_traceback\n";
    assert_builtin(source, "args", "args = None");
    assert_builtin(
        source,
        "with_traceback",
        "def with_traceback(self, *args, **kwargs): ...",
    );
}

#[test]
fn builtin_member_through_three_levels() {
    // `KeyError` derives from `LookupError`, `Exception` and `BaseException`.
    let source = "class MyKeyError(KeyError):\n    pass\n\nx = MyKeyError().args\n";
    assert_builtin(source, "args", "args = None");
    assert_builtin("x = KeyError().args\n", "args", "args = None");
}

#[test]
fn builtin_class_member_through_user_subclass() {
    let source = "class MyError(Exception):\n    pass\n\nx = MyError.with_traceback\n";
    assert_builtin(
        source,
        "with_traceback",
        "def with_traceback(self, *args, **kwargs): ...",
    );
}

#[test]
fn builtin_member_through_self_in_subclass() {
    let source =
        "class MyError(Exception):\n    def show(self):\n        return self.args\n";
    assert_builtin(source, "args", "args = None");
}

#[test]
fn builtin_module_constant() {
    assert_builtin(
        "if __name__ == '__main__':\n    pass\n",
        "__name__",
        "__name__ = None",
    );
}

#[test]
fn function_definition_shadows_builtin() {
    let targets = resolve("def len(x):\n    pass\n\ny = len(1)\n", "len");
    assert_eq!(targets, vec![Target::Local(0)]);
}

#[test]
fn class_definition_shadows_builtin() {
    let targets = resolve("class dict:\n    pass\n\ny = dict()\n", "dict");
    assert_eq!(targets, vec![Target::Local(0)]);
}

#[test]
fn nested_definition_shadows_builtin() {
    let source = "def f():\n    def len():\n        pass\n    return len\n";
    assert_eq!(resolve(source, "len"), vec![Target::Local(1)]);
}

#[test]
fn parameter_shadows_builtin() {
    let source = "def f(id, *iter, type=1, **min):\n    return id, iter, type, min\n";
    for name in ["id", "iter", "type", "min"] {
        assert_eq!(resolve(source, name), vec![Target::Local(0)], "{}", name);
    }
}

#[test]
fn method_parameter_shadows_builtin() {
    let source = "class A:\n    def m(self, id):\n        return id\n";
    assert_eq!(resolve(source, "id"), vec![Target::Local(1)]);
}

#[test]
fn lambda_parameter_shadows_builtin() {
    let targets = resolve("f = lambda id: id\n", "id");
    assert_eq!(targets, vec![Target::Local(0)]);
}

#[test]
fn local_variable_shadows_builtin() {
    let source = "def f():\n    id = 1\n    return id\n";
    assert_eq!(resolve(source, "id"), vec![Target::Local(1)]);
}

#[test]
fn conditional_binding_keeps_the_builtin() {
    // The assignment may not run, so both the local variable and the builtin can be the target.
    let source = "def f(a):\n    if a:\n        id = 1\n    return id\n";
    let targets = resolve(source, "id");
    assert_eq!(targets.len(), 2, "{:?}", targets);
    assert!(targets.contains(&Target::Local(2)), "{:?}", targets);
}
