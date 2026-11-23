// -*- coding: utf-8 -*-
// ------------------------------------------------------------------------------------------------
// Copyright © 2026, stack-graphs authors.
// Licensed under either of Apache License, Version 2.0, or MIT license, at your option.
// Please see the LICENSE-APACHE or LICENSE-MIT files in this distribution for license details.
// ------------------------------------------------------------------------------------------------

//! Tests for the marker of a union that the code declares.
//!
//! A union annotation, a quoted union, and `Union[...]` give a scope node the syntax type
//! "union". A path from a member of the union passes this node. A consumer reads the syntax type
//! to tell a declared union from a union that it infers.

use stack_graphs::graph::StackGraph;
use tree_sitter_stack_graphs::NoCancellation;
use tree_sitter_stack_graphs::Variables;

const TEST_FILE: &str = "test.py";

/// The number of nodes of the test file that have the syntax type "union".
fn union_nodes(source: &str) -> usize {
    let lc = tree_sitter_stack_graphs_python::language_configuration(&NoCancellation);
    let mut graph = StackGraph::new();
    let file = graph.get_or_create_file(TEST_FILE);
    let mut globals = Variables::new();
    globals
        .add("FILE_PATH".into(), TEST_FILE.into())
        .expect("set FILE_PATH");
    lc.sgl
        .build_stack_graph_into(&mut graph, file, source, &globals, &NoCancellation)
        .expect("build test source");
    graph
        .nodes_for_file(file)
        .filter(|n| {
            graph
                .source_info(*n)
                .and_then(|info| info.syntax_type.into_option())
                .is_some_and(|t| &graph[t] == "union")
        })
        .count()
}

const CLASSES: &str = "class A: ...\nclass B: ...\nclass C: ...\n";

fn annotated(annotation: &str) -> String {
    format!("{CLASSES}def f(x: {annotation}):\n    return x.m\n")
}

#[test]
fn a_union_with_a_bar_has_the_marker() {
    assert!(union_nodes(&annotated("A | B")) >= 1);
    assert!(union_nodes(&annotated("A | B | C | None")) >= 1);
}

#[test]
fn a_union_of_the_typing_module_has_the_marker() {
    assert!(union_nodes(&annotated("Union[A, B]")) >= 1);
    assert!(union_nodes(&annotated("Optional[Union[A, B]]")) >= 1);
}

#[test]
fn a_quoted_union_has_the_marker() {
    assert!(union_nodes(&annotated("\"A | B\"")) >= 1);
    assert!(union_nodes(&annotated("\"Union[A, B, C]\"")) >= 1);
}

#[test]
fn a_union_alias_has_the_marker() {
    let source = format!("{CLASSES}X = A | B\ndef f(x: X):\n    return x.m\n");
    assert!(union_nodes(&source) >= 1);
}

#[test]
fn a_plain_name_and_an_optional_name_have_no_marker() {
    assert_eq!(union_nodes(&annotated("A")), 0);
    assert_eq!(union_nodes(&annotated("Optional[A]")), 0);
}

#[test]
fn an_inferred_union_has_no_marker() {
    let source = format!(
        "{CLASSES}def f(flag):\n    x = A() if flag else B()\n    y = x or C()\n    return y.m\n"
    );
    assert_eq!(union_nodes(&source), 0);
}
