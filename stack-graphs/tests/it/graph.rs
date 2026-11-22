// -*- coding: utf-8 -*-
// ------------------------------------------------------------------------------------------------
// Copyright © 2021, stack-graphs authors.
// Licensed under either of Apache License, Version 2.0, or MIT license, at your option.
// Please see the LICENSE-APACHE or LICENSE-MIT files in this distribution for license details.
// ------------------------------------------------------------------------------------------------

use std::collections::HashSet;

use maplit::hashset;
use stack_graphs::graph::{Degree, NodeID, StackGraph};

use crate::test_graphs;
use crate::test_graphs::CreateStackGraph;

#[test]
fn can_create_symbols() {
    let mut graph = StackGraph::new();
    let a1 = graph.add_symbol("a");
    let a2 = graph.add_symbol("a");
    let b = graph.add_symbol("b");
    let c = graph.add_symbol("c");
    let empty1 = graph.add_symbol("");
    // The content of each symbol be comparable
    assert_eq!(graph[a1], graph[a2]);
    assert_ne!(graph[a1], graph[b]);
    assert_ne!(graph[a1], graph[c]);
    assert_ne!(graph[a2], graph[b]);
    assert_ne!(graph[a2], graph[c]);
    assert_ne!(graph[b], graph[c]);
    assert_ne!(graph[empty1], graph[a1]);
    // and because we deduplicate symbols, the handles should be comparable too.
    assert_eq!(a1, a2);
    assert_ne!(a1, b);
    assert_ne!(a1, c);
    assert_ne!(a2, b);
    assert_ne!(a2, c);
    assert_ne!(b, c);
    assert_ne!(empty1, a1);
}

#[test]
fn can_iterate_symbols() {
    let mut graph = StackGraph::new();
    graph.add_symbol("a");
    graph.add_symbol("b");
    graph.add_symbol("c");
    // We should get all of the symbols that we've created — though there's no guarantee in which
    // order they'll come out of the iterator.
    let symbols = graph
        .iter_symbols()
        .map(|symbol| &graph[symbol])
        .collect::<HashSet<_>>();
    assert_eq!(symbols, hashset! {"a", "b", "c"});
}

#[test]
fn can_display_symbols() {
    let mut graph = StackGraph::new();
    graph.add_symbol("a");
    graph.add_symbol("b");
    graph.add_symbol("c");
    let mut symbols = graph
        .iter_symbols()
        .map(|symbol| symbol.display(&graph).to_string())
        .collect::<Vec<_>>();
    symbols.sort();
    assert_eq!(symbols, vec!["a", "b", "c"]);
}

#[test]
fn can_create_strings() {
    let mut graph = StackGraph::new();
    let a1 = graph.add_string("a");
    let a2 = graph.add_string("a");
    let b = graph.add_string("b");
    let c = graph.add_string("c");
    let empty1 = graph.add_string("");
    // The content of each string be comparable
    assert_eq!(graph[a1], graph[a2]);
    assert_ne!(graph[a1], graph[b]);
    assert_ne!(graph[a1], graph[c]);
    assert_ne!(graph[a2], graph[b]);
    assert_ne!(graph[a2], graph[c]);
    assert_ne!(graph[b], graph[c]);
    assert_ne!(graph[empty1], graph[a1]);
    // and because we deduplicate strings, the handles should be comparable too.
    assert_eq!(a1, a2);
    assert_ne!(a1, b);
    assert_ne!(a1, c);
    assert_ne!(a2, b);
    assert_ne!(a2, c);
    assert_ne!(b, c);
    assert_ne!(empty1, a1);
}

#[test]
fn can_iterate_strings() {
    let mut graph = StackGraph::new();
    graph.add_string("a");
    graph.add_string("b");
    graph.add_string("c");
    // We should get all of the strings that we've created — though there's no guarantee in which
    // order they'll come out of the iterator.
    let strings = graph
        .iter_strings()
        .map(|string| &graph[string])
        .collect::<HashSet<_>>();
    assert_eq!(strings, hashset! {"a", "b", "c"});
}

#[test]
fn can_display_strings() {
    let mut graph = StackGraph::new();
    graph.add_string("a");
    graph.add_string("b");
    graph.add_string("c");
    let mut strings = graph
        .iter_strings()
        .map(|string| string.display(&graph).to_string())
        .collect::<Vec<_>>();
    strings.sort();
    assert_eq!(strings, vec!["a", "b", "c"]);
}

#[test]
fn can_iterate_nodes() {
    let mut graph = StackGraph::new();
    let file = graph.get_or_create_file("test.py");
    let h1 = graph.internal_scope(file, 0);
    let h2 = graph.internal_scope(file, 1);
    let h3 = graph.internal_scope(file, 2);
    let handles = graph.iter_nodes().collect::<HashSet<_>>();
    assert_eq!(
        handles,
        hashset! {graph.root_node(), graph.jump_to_node(), h1, h2, h3}
    );
}

#[test]
fn can_add_and_remove_edges() {
    let mut graph = StackGraph::new();
    let file = graph.get_or_create_file("test.py");
    let h1 = graph.internal_scope(file, 0);
    let h2 = graph.internal_scope(file, 1);
    let h3 = graph.internal_scope(file, 2);
    let h4 = graph.internal_scope(file, 3);
    graph.add_edge(h1, h2, 0);
    graph.add_edge(h1, h3, 0);
    graph.add_edge(h1, h4, 0);
    // If you try to overwrite an edge, the original edge takes precedence.
    graph.add_edge(h1, h3, 1);
    assert_eq!(
        graph
            .outgoing_edges(h1)
            .map(|edge| (edge.sink, edge.precedence))
            .collect::<HashSet<_>>(),
        hashset! { (h2, 0), (h3, 0), (h4, 0) }
    );
}

#[test]
fn singleton_nodes_have_correct_ids() {
    let graph = StackGraph::new();
    let root_handle = StackGraph::root_node();
    let root = &graph[root_handle];
    assert!(root.is_root());
    assert!(root.id().is_root());
    assert_eq!(root.display(&graph).to_string(), "[root]");
    assert_eq!(root.id().display(&graph).to_string(), "[root]");
}

#[test]
fn can_add_graph_to_empty_graph() {
    let mut graph = StackGraph::new();
    let other = test_graphs::simple::new();
    graph.add_from_graph(&other).expect("Adding graph failed");

    for other_file in other.iter_files() {
        let file = graph
            .get_file(other[other_file].name())
            .expect("Missing file");
        assert_eq!(
            graph.nodes_for_file(file).count(),
            other.nodes_for_file(other_file).count()
        );
        assert_eq!(
            graph
                .nodes_for_file(file)
                .map(|n| graph.outgoing_edges(n).count())
                .sum::<usize>(),
            other
                .nodes_for_file(other_file)
                .map(|n| graph.outgoing_edges(n).count())
                .sum::<usize>()
        );
    }
}

#[test]
fn can_add_graph_with_root_edges_to_several_files() {
    // The root node has an edge to a node of each file.
    let mut other = StackGraph::new();
    let file_a = other.get_or_create_file("a.py");
    let file_b = other.get_or_create_file("b.py");
    let a = other.internal_scope(file_a, 0);
    let b = other.internal_scope(file_b, 0);
    other.add_edge(StackGraph::root_node(), a, 0);
    other.add_edge(StackGraph::root_node(), b, 0);

    let mut graph = StackGraph::new();
    graph.add_from_graph(&other).expect("Adding graph failed");
    assert_eq!(
        graph.outgoing_edges(StackGraph::root_node()).count(),
        2,
        "the root node must keep its edges to the nodes of both files"
    );
}

#[test]
fn can_get_incoming_edges() {
    let mut graph = StackGraph::new();
    let file = graph.get_or_create_file("test.py");
    let h1 = graph.internal_scope(file, 0);
    let h2 = graph.internal_scope(file, 1);
    let h3 = graph.internal_scope(file, 2);
    assert_eq!(Degree::Zero, graph.incoming_edge_degree(h1));
    assert_eq!(Degree::Zero, graph.incoming_edge_degree(h2));
    assert_eq!(Degree::Zero, graph.incoming_edge_degree(h3));
    graph.add_edge(h1, h2, 0);
    graph.add_edge(h3, h2, 0);
    assert_eq!(Degree::Zero, graph.incoming_edge_degree(h1));
    assert_eq!(Degree::Multiple, graph.incoming_edge_degree(h2));
    assert_eq!(Degree::Zero, graph.incoming_edge_degree(h3));
    graph.add_edge(h3, h1, 0);
    assert_eq!(Degree::One, graph.incoming_edge_degree(h1));
}

fn span(line: usize, start: usize, end: usize) -> lsp_positions::Span {
    let position = |column: usize| lsp_positions::Position {
        line,
        column: lsp_positions::Offset {
            utf8_offset: column,
            utf16_offset: column + 1,
            grapheme_offset: column + 2,
        },
        containing_line: 100 * (line % 1000)..100 * (line % 1000) + 90,
        trimmed_line: 100 * (line % 1000) + 4..100 * (line % 1000) + 80,
    };
    lsp_positions::Span {
        start: position(start),
        end: position(end),
    }
}

#[test]
fn source_info_keeps_every_field() {
    let mut graph = StackGraph::new();
    let file = graph.get_or_create_file("test.py");
    let a = graph.add_scope_node(NodeID::new_in_file(file, 0), false).unwrap();
    let b = graph.add_scope_node(NodeID::new_in_file(file, 1), false).unwrap();
    let c = graph.add_scope_node(NodeID::new_in_file(file, 2), false).unwrap();
    let d = graph.add_scope_node(NodeID::new_in_file(file, 3), false).unwrap();
    let syntax_type = graph.add_string("function");
    let line = graph.add_string("def f(): pass");
    let name = graph.add_string("m.f");

    // A node after the last node with source info has none.
    assert!(graph.source_info(d).is_none());

    {
        let mut info = graph.source_info_mut(b);
        info.span = span(3, 4, 9);
        info.syntax_type = syntax_type.into();
        info.containing_line = line.into();
        info.definiens_span = span(3, 12, 20);
        info.fully_qualified_name = name.into();
    }
    // A node below the last node with source info reads as the default value.
    let default = graph.source_info(a).unwrap();
    assert_eq!(default.span, lsp_positions::Span::default());
    assert!(default.syntax_type.into_option().is_none());

    let info = graph.source_info(b).unwrap();
    assert_eq!(info.span, span(3, 4, 9));
    assert_eq!(info.definiens_span, span(3, 12, 20));
    assert_eq!(info.syntax_type.into_option(), Some(syntax_type));
    assert_eq!(info.containing_line.into_option(), Some(line));
    assert_eq!(info.fully_qualified_name.into_option(), Some(name));
    assert_eq!(graph.syntax_type(b), Some(syntax_type));
    assert_eq!(graph.syntax_type(a), None);

    // A change of one field keeps the others, and a definiens span can go back to none.
    graph.source_info_mut(b).span = span(5, 0, 2);
    graph.source_info_mut(b).definiens_span = lsp_positions::Span::default();
    let info = graph.source_info(b).unwrap();
    assert_eq!(info.span, span(5, 0, 2));
    assert_eq!(info.definiens_span, lsp_positions::Span::default());
    assert_eq!(info.syntax_type.into_option(), Some(syntax_type));
    graph.source_info_mut(b).definiens_span = span(6, 1, 2);
    assert_eq!(graph.source_info(b).unwrap().definiens_span, span(6, 1, 2));

    // A value that does not fit in 32 bits keeps its value.
    let huge = usize::MAX / 2;
    *graph.source_info_mut(c) = stack_graphs::graph::SourceInfo {
        span: span(huge, 1, 2),
        syntax_type: syntax_type.into(),
        ..Default::default()
    };
    assert_eq!(graph.source_info(c).unwrap().span, span(huge, 1, 2));
    assert_eq!(graph.syntax_type(c), Some(syntax_type));
    graph.source_info_mut(c).span = span(1, 1, 2);
    assert_eq!(graph.source_info(c).unwrap().span, span(1, 1, 2));
    assert_eq!(graph.syntax_type(c), Some(syntax_type));
}
