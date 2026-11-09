// -*- coding: utf-8 -*-
// ------------------------------------------------------------------------------------------------
// Copyright © 2024, stack-graphs authors.
// Licensed under either of Apache License, Version 2.0, or MIT license, at your option.
// Please see the LICENSE-APACHE or LICENSE-MIT files in this distribution for license details.
// ------------------------------------------------------------------------------------------------

//! Tests for the limits of a stitcher run, and for scoped resolution.

use stack_graphs::arena::Handle;
use stack_graphs::graph::Node;
use stack_graphs::graph::StackGraph;
use stack_graphs::partial::PartialPaths;
use stack_graphs::stitching::ForwardPartialPathStitcher;
use stack_graphs::stitching::GraphEdgeCandidates;
use stack_graphs::stitching::StitchBound;
use stack_graphs::stitching::StitcherConfig;
use stack_graphs::stitching::Stats;
use stack_graphs::NoCancellation;

use crate::util::create_pop_symbol_node;
use crate::util::create_push_symbol_node;
use crate::util::create_scope_node;

/// Builds `reference -> s0 -> ... -> sN -> definition`.  Between `s(i)` and `s(i+1)`, there are
/// two parallel routes.  The graph has `2^N` complete paths.  Similar path detection prunes
/// most of them, so tests that want the blow up must disable it.
fn diamond_chain(diamonds: usize) -> (StackGraph, Handle<Node>) {
    let mut graph = StackGraph::new();
    let file = graph.add_file("diamonds").unwrap();
    let reference = create_push_symbol_node(&mut graph, file, "x", true);
    let mut previous = create_scope_node(&mut graph, file, false);
    graph.add_edge(reference, previous, 0);
    for _ in 0..diamonds {
        let left = create_scope_node(&mut graph, file, false);
        let right = create_scope_node(&mut graph, file, false);
        let next = create_scope_node(&mut graph, file, false);
        for side in [left, right] {
            graph.add_edge(previous, side, 0);
            graph.add_edge(side, next, 0);
        }
        previous = next;
    }
    let definition = create_pop_symbol_node(&mut graph, file, "x", true);
    graph.add_edge(previous, definition, 0);
    (graph, reference)
}

/// Builds `reference -> scope(i) -> definition` for each `i` in `0..width`.
fn fan_out(width: usize) -> (StackGraph, Handle<Node>) {
    let mut graph = StackGraph::new();
    let file = graph.add_file("fan").unwrap();
    let reference = create_push_symbol_node(&mut graph, file, "x", true);
    let definition = create_pop_symbol_node(&mut graph, file, "x", true);
    for _ in 0..width {
        let scope = create_scope_node(&mut graph, file, false);
        graph.add_edge(reference, scope, 0);
        graph.add_edge(scope, definition, 0);
    }
    (graph, reference)
}

/// Builds `reference -> s0 -> ... -> s(length-1) -> definition`.
fn chain(length: usize) -> (StackGraph, Handle<Node>) {
    let mut graph = StackGraph::new();
    let file = graph.add_file("chain").unwrap();
    let reference = create_push_symbol_node(&mut graph, file, "x", true);
    let mut previous = reference;
    for _ in 0..length {
        let scope = create_scope_node(&mut graph, file, false);
        graph.add_edge(previous, scope, 0);
        previous = scope;
    }
    let definition = create_pop_symbol_node(&mut graph, file, "x", true);
    graph.add_edge(previous, definition, 0);
    (graph, reference)
}

/// Runs a stitcher from the reference, directly on the graph.  Returns the number of complete
/// paths and the stats.
fn run(graph: &StackGraph, reference: Handle<Node>, config: StitcherConfig) -> (usize, Stats) {
    let mut partials = PartialPaths::new();
    let mut count = 0;
    let stats = ForwardPartialPathStitcher::find_all_complete_partial_paths(
        &mut GraphEdgeCandidates::new(graph, &mut partials, None),
        std::iter::once(reference),
        config,
        &NoCancellation,
        |_, _, _| count += 1,
    )
    .expect("should never be cancelled");
    (count, stats)
}

#[test]
fn default_config_has_no_limits() {
    let (graph, reference) = diamond_chain(3);
    let config = StitcherConfig::default().with_detect_similar_paths(false);
    let (count, stats) = run(&graph, reference, config);
    assert_eq!(8, count);
    assert_eq!(None, stats.bounded);
    assert!(stats.work > 0);
}

#[test]
fn generous_limits_do_not_change_the_result() {
    let (graph, reference) = diamond_chain(3);
    let config = StitcherConfig::default().with_detect_similar_paths(false);
    let limited = config
        .with_max_work(Some(1_000_000))
        .with_max_queue_len(Some(1_000_000))
        .with_max_arena_cells(Some(1_000_000))
        .with_max_path_length(Some(1_000));
    let (count, stats) = run(&graph, reference, config);
    let (limited_count, limited_stats) = run(&graph, reference, limited);
    assert_eq!(count, limited_count);
    assert_eq!(None, limited_stats.bounded);
    assert_eq!(stats.work, limited_stats.work);
}

#[test]
fn work_limit_stops_an_exploding_graph() {
    // Without the limit, this run builds 2^30 paths.
    let (graph, reference) = diamond_chain(30);
    let config = StitcherConfig::default()
        .with_detect_similar_paths(false)
        .with_max_work(Some(10_000));
    let (count, stats) = run(&graph, reference, config);
    assert_eq!(Some(StitchBound::Work), stats.bounded);
    assert_eq!(0, count);
    assert!(stats.work > 10_000);
    assert!(stats.work < 100_000);
}

#[test]
fn queue_limit_stops_a_wide_graph() {
    // Similar path detection would merge the paths, so turn it off.
    let (graph, reference) = fan_out(1_000);
    let config = StitcherConfig::default().with_detect_similar_paths(false);
    let (count, stats) = run(&graph, reference, config);
    assert_eq!(1_000, count);
    assert_eq!(None, stats.bounded);
    assert!(stats.peak_queue_len >= 1_000);

    let (_, stats) = run(&graph, reference, config.with_max_queue_len(Some(100)));
    assert_eq!(Some(StitchBound::QueueLength), stats.bounded);
}

#[test]
fn arena_limit_stops_a_deep_graph() {
    let (graph, reference) = chain(200);
    let (count, stats) = run(&graph, reference, StitcherConfig::default());
    assert_eq!(1, count);
    assert_eq!(None, stats.bounded);
    assert!(stats.arena_cells_added > 200);

    let config = StitcherConfig::default().with_max_arena_cells(Some(50));
    let (count, stats) = run(&graph, reference, config);
    assert_eq!(0, count);
    assert_eq!(Some(StitchBound::ArenaCells), stats.bounded);
}

#[test]
fn path_length_limit_drops_long_paths() {
    let (graph, reference) = chain(50);
    let config = StitcherConfig::default().with_max_path_length(Some(100));
    let (count, stats) = run(&graph, reference, config);
    assert_eq!(1, count);
    assert_eq!(None, stats.bounded);

    let config = StitcherConfig::default().with_max_path_length(Some(10));
    let (count, stats) = run(&graph, reference, config);
    assert_eq!(0, count);
    assert_eq!(Some(StitchBound::PathLength), stats.bounded);
}

#[test]
fn a_bounded_run_keeps_the_paths_it_found() {
    // The short route reaches the definition in a few phases.  The long route runs out of work.
    let mut graph = StackGraph::new();
    let file = graph.add_file("two-routes").unwrap();
    let reference = create_push_symbol_node(&mut graph, file, "x", true);
    let definition = create_pop_symbol_node(&mut graph, file, "x", true);
    let short = create_scope_node(&mut graph, file, false);
    graph.add_edge(reference, short, 0);
    graph.add_edge(short, definition, 0);
    let mut previous = reference;
    for _ in 0..100 {
        let scope = create_scope_node(&mut graph, file, false);
        graph.add_edge(previous, scope, 0);
        previous = scope;
    }
    let config = StitcherConfig::default().with_max_work(Some(50));
    let (count, stats) = run(&graph, reference, config);
    assert_eq!(1, count);
    assert_eq!(Some(StitchBound::Work), stats.bounded);
}
