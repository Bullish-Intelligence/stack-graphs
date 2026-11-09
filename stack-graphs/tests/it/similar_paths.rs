// -*- coding: utf-8 -*-
// ------------------------------------------------------------------------------------------------
// Copyright © 2024, stack-graphs authors.
// Licensed under either of Apache License, Version 2.0, or MIT license, at your option.
// Please see the LICENSE-APACHE or LICENSE-MIT files in this distribution for license details.
// ------------------------------------------------------------------------------------------------

//! Tests for shadow aware similar path detection.

use stack_graphs::graph::StackGraph;
use stack_graphs::partial::PartialPaths;
use stack_graphs::stitching::ForwardPartialPathStitcher;
use stack_graphs::stitching::StitcherConfig;
use stack_graphs::NoCancellation;

use crate::util::create_pop_symbol_node;
use crate::util::create_push_symbol_node;
use crate::util::create_scope_node;

/// Builds the scopes of `x = 1; if c: x = 2 else: <other>; x`.
///
/// ```text
/// reference -> after -> left  -> binding         (precedence 1)
///                             -> join
///                    -> right -> other binding  (precedence 1)
///                             -> join
/// join -> earlier binding
/// ```
///
/// The left branch binds `x`.  If `right_binds_x`, the right branch binds `x`, too.  Otherwise the
/// right branch binds another name.  Both branches pass the earlier binding through to `join`.
/// The graph keeps no edge that skips the branches.  `left_first` selects which branch the graph
/// lists first.
///
/// Returns the symbols of the definitions that complete paths reach, after the removal of the
/// paths that another path shadows.
fn reached(config: StitcherConfig, left_first: bool, right_binds_x: bool) -> Vec<String> {
    let mut graph = StackGraph::new();
    let file = graph.add_file("if-else").unwrap();
    let reference = create_push_symbol_node(&mut graph, file, "x", true);
    let after = create_scope_node(&mut graph, file, false);
    let left = create_scope_node(&mut graph, file, false);
    let right = create_scope_node(&mut graph, file, false);
    let join = create_scope_node(&mut graph, file, false);
    let left_binding = create_pop_symbol_node(&mut graph, file, "x", true);
    let right_binding = create_pop_symbol_node(
        &mut graph,
        file,
        if right_binds_x { "x" } else { "y" },
        true,
    );
    let earlier = create_pop_symbol_node(&mut graph, file, "x", true);
    graph.add_edge(reference, after, 0);
    let mut branches = vec![(left, left_binding), (right, right_binding)];
    if !left_first {
        branches.reverse();
    }
    for (branch, binding) in branches {
        graph.add_edge(after, branch, 0);
        graph.add_edge(branch, binding, 1);
        graph.add_edge(branch, join, 0);
    }
    graph.add_edge(join, earlier, 0);

    let mut partials = PartialPaths::new();
    let mut paths = Vec::new();
    ForwardPartialPathStitcher::find_minimal_partial_path_set_in_file(
        &graph,
        &mut partials,
        file,
        config,
        &NoCancellation,
        |_, _, path| paths.push(path.clone()),
    )
    .expect("should never be cancelled");

    let mut found = Vec::new();
    for (index, path) in paths.iter().enumerate() {
        if !path.starts_at_reference(&graph) || !path.ends_at_definition(&graph) {
            continue;
        }
        let shadowed = paths
            .iter()
            .enumerate()
            .any(|(other, o)| other != index && o.shadows(&mut partials, path));
        if !shadowed {
            let end = graph[path.end_node].id().local_id();
            found.push(format!("{}", end));
        }
    }
    found.sort();
    found.dedup();
    found
}

#[test]
fn default_detection_can_drop_the_path_that_a_branch_without_binding_gives() {
    // The local ids of the nodes are: reference 0, after 1, left 2, right 3, join 4, left binding 5,
    // right binding 6, earlier binding 7.  The expected result has the left binding, and the
    // earlier binding through the right branch.
    let default = StitcherConfig::default();
    let one_order = reached(default, true, false);
    let other_order = reached(default, false, false);
    // One of the two orders hides the earlier binding.
    assert!(
        one_order.len() == 1 || other_order.len() == 1,
        "default detection lost nothing: {:?} {:?}",
        one_order,
        other_order
    );
}

#[test]
fn shadow_aware_detection_keeps_the_path_of_a_branch_without_binding() {
    let config = StitcherConfig::default().with_shadow_aware_similar_paths(true);
    for left_first in [true, false] {
        // The right branch binds another name, so the earlier binding stays reachable.
        assert_eq!(
            vec!["5".to_string(), "7".to_string()],
            reached(config, left_first, false),
            "left_first = {}",
            left_first
        );
    }
}

#[test]
fn shadow_aware_detection_drops_the_earlier_binding_if_every_branch_binds() {
    let config = StitcherConfig::default().with_shadow_aware_similar_paths(true);
    for left_first in [true, false] {
        assert_eq!(
            vec!["5".to_string(), "6".to_string()],
            reached(config, left_first, true),
            "left_first = {}",
            left_first
        );
    }
}
