// -*- coding: utf-8 -*-
// ------------------------------------------------------------------------------------------------
// Copyright © 2024, stack-graphs authors.
// Licensed under either of Apache License, Version 2.0, or MIT license, at your option.
// Please see the LICENSE-APACHE or LICENSE-MIT files in this distribution for license details.
// ------------------------------------------------------------------------------------------------

//! Tests for the limits of a stitcher run, and for scoped resolution.

use std::collections::BTreeSet;

use stack_graphs::arena::Handle;
use stack_graphs::graph::Node;
use stack_graphs::graph::StackGraph;
use stack_graphs::partial::PartialPath;
use stack_graphs::partial::PartialPaths;
use stack_graphs::stitching::Database;
use stack_graphs::stitching::DatabaseCandidates;
use stack_graphs::stitching::ForwardPartialPathStitcher;
use stack_graphs::stitching::GraphEdgeCandidates;
use stack_graphs::stitching::StitchBound;
use stack_graphs::stitching::StitcherConfig;
use stack_graphs::stitching::Stats;
use stack_graphs::NoCancellation;

use crate::test_graphs;
use crate::util::create_pop_scoped_symbol_node;
use crate::util::create_pop_symbol_node;
use crate::util::create_push_scoped_symbol_node;
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

fn database_for(graph: &StackGraph, partials: &mut PartialPaths) -> Database {
    let mut db = Database::new();
    for file in graph.iter_files() {
        ForwardPartialPathStitcher::find_minimal_partial_path_set_in_file(
            graph,
            partials,
            file,
            StitcherConfig::default(),
            &NoCancellation,
            |graph, partials, path| {
                db.add_partial_path(graph, partials, path.clone());
            },
        )
        .expect("should never be cancelled");
    }
    db
}

fn resolve_all(
    graph: &StackGraph,
    partials: &mut PartialPaths,
    db: &Database,
    scoped: bool,
    config: StitcherConfig,
) -> BTreeSet<String> {
    let mut results = BTreeSet::new();
    let references = graph
        .iter_nodes()
        .filter(|handle| graph[*handle].is_reference())
        .collect::<Vec<_>>();
    for reference in references {
        let visit = |graph: &StackGraph, partials: &mut PartialPaths, path: &PartialPath| {
            results.insert(path.display(graph, partials).to_string());
        };
        if scoped {
            ForwardPartialPathStitcher::find_complete_partial_paths_in_scope(
                graph,
                partials,
                db,
                reference,
                config,
                &NoCancellation,
                visit,
            )
            .expect("should never be cancelled");
        } else {
            ForwardPartialPathStitcher::find_all_complete_partial_paths(
                &mut DatabaseCandidates::new(graph, partials, db),
                std::iter::once(reference),
                config,
                &NoCancellation,
                visit,
            )
            .expect("should never be cancelled");
        }
    }
    results
}

#[test]
fn scoped_resolution_matches_plain_resolution_and_releases_memory() {
    let graph = test_graphs::class_field_through_function_parameter::new::<StackGraph>();
    let mut partials = PartialPaths::new();
    let db = database_for(&graph, &mut partials);

    let before = partials.allocated_cells();
    let first = resolve_all(&graph, &mut partials, &db, true, StitcherConfig::default());
    assert_eq!(before, partials.allocated_cells());
    assert_eq!(6, first.len());

    // The scope must also undo changes to older cells.  A second pass and a plain pass must
    // see the same results.
    let second = resolve_all(&graph, &mut partials, &db, true, StitcherConfig::default());
    assert_eq!(before, partials.allocated_cells());
    assert_eq!(first, second);

    let plain = resolve_all(&graph, &mut partials, &db, false, StitcherConfig::default());
    assert!(partials.allocated_cells() > before);
    assert_eq!(first, plain);
}

#[test]
fn scoped_resolution_releases_memory_of_a_bounded_run() {
    let graph = test_graphs::class_field_through_function_parameter::new::<StackGraph>();
    let mut partials = PartialPaths::new();
    let db = database_for(&graph, &mut partials);
    let expected = resolve_all(&graph, &mut partials, &db, true, StitcherConfig::default());

    let before = partials.allocated_cells();
    let tight = StitcherConfig::default().with_max_work(Some(1));
    let bounded = resolve_all(&graph, &mut partials, &db, true, tight);
    assert_eq!(before, partials.allocated_cells());
    assert!(bounded.len() < expected.len());

    // After the bounded runs, a normal run still finds everything.
    let again = resolve_all(&graph, &mut partials, &db, true, StitcherConfig::default());
    assert_eq!(expected, again);
}

#[test]
fn scopes_release_memory_on_panic() {
    let mut partials = PartialPaths::new();
    let before = partials.allocated_cells();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        partials.with_scope(|_| panic!("stop"));
    }));
    assert!(result.is_err());
    assert_eq!(before, partials.allocated_cells());
    // The checkpoint is gone, so a new scope works.
    partials.with_scope(|_| ());
}

/// Builds a chain of `levels` calls. Each level has two call sites. A call site pushes the symbol
/// `()` with its own scope, and the callee pops it right away. The scope stack of a path grows by
/// one scope at each level, so the paths differ in all their call sites. Like `diamond_chain`,
/// the graph has `2^N` routes, but now no two routes carry the same scope stack.
///
/// With `jumps`, the reference reaches the definition only after one jump to a scope for each level.
/// The jumps go from the newest call site to the oldest one. The definition sits behind the
/// call sites of the first level.
fn call_chain(levels: usize, jumps: bool) -> (StackGraph, Handle<Node>) {
    let mut graph = StackGraph::new();
    let file = graph.add_file("calls").unwrap();
    let reference = create_push_symbol_node(&mut graph, file, "x", true);
    let mut previous = reference;
    let mut first_sites = Vec::new();
    for level in 0..levels {
        let pop = create_pop_scoped_symbol_node(&mut graph, file, "()", false);
        for _ in 0..2 {
            let site = create_scope_node(&mut graph, file, true);
            let site_id = graph[site].id();
            let push = create_push_scoped_symbol_node(&mut graph, file, "()", site_id, false);
            graph.add_edge(previous, push, 0);
            graph.add_edge(push, pop, 0);
            if level == 0 {
                first_sites.push(site);
            }
            if jumps {
                graph.add_edge(site, StackGraph::jump_to_node(), 0);
            }
        }
        previous = pop;
    }
    let definition = create_pop_symbol_node(&mut graph, file, "x", true);
    if jumps {
        graph.add_edge(previous, StackGraph::jump_to_node(), 0);
        for site in first_sites {
            graph.add_edge(site, definition, 0);
        }
    } else {
        graph.add_edge(previous, definition, 0);
    }
    (graph, reference)
}

#[test]
fn without_a_scope_depth_limit_call_chains_grow_exponentially() {
    for levels in [4, 6, 8] {
        let (graph, reference) = call_chain(levels, false);
        let (count, stats) = run(&graph, reference, StitcherConfig::default());
        assert_eq!(1usize << levels, count);
        assert!(stats.peak_queue_len >= 1 << levels);
    }
}

#[test]
fn a_scope_depth_limit_stops_the_growth_of_call_chains() {
    let config = StitcherConfig::default().with_max_scope_depth(Some(1));
    let mut peaks = Vec::new();
    for levels in [4, 8, 16, 32] {
        let (graph, reference) = call_chain(levels, false);
        let (count, stats) = run(&graph, reference, config);
        // The paths that differ only in old call sites are similar. The detector keeps few.
        assert!(count >= 1 && count <= 2);
        assert_eq!(None, stats.bounded);
        peaks.push(stats.peak_queue_len);
    }
    assert!(
        peaks.iter().all(|peak| *peak <= peaks[0] + 2),
        "{}",
        format!("{peaks:?}")
    );

    let (graph, reference) = call_chain(8, false);
    let config = StitcherConfig::default().with_max_scope_depth(Some(2));
    let (_, stats) = run(&graph, reference, config);
    assert!(stats.peak_queue_len < 64);
}

#[test]
fn a_scope_depth_limit_loses_results_beyond_its_depth_and_adds_none() {
    let (graph, reference) = call_chain(3, true);
    // The detector merges the routes that reach the same definition, so few paths remain.
    let (count, _) = run(&graph, reference, StitcherConfig::default());
    assert!(count >= 1);
    // A jump to a scope that the limit dropped finds no scope, so the path ends.
    for depth in [1, 2] {
        let config = StitcherConfig::default().with_max_scope_depth(Some(depth));
        let (count, stats) = run(&graph, reference, config);
        assert_eq!(0, count, "depth {depth}");
        assert_eq!(None, stats.bounded);
    }
    // The limit is deep enough for the jumps. The limit merges the routes that it can.
    let config = StitcherConfig::default().with_max_scope_depth(Some(3));
    let (count, _) = run(&graph, reference, config);
    assert!(count >= 1);
}

#[test]
fn a_scope_depth_limit_that_the_paths_do_not_reach_changes_nothing() {
    let (graph, reference) = call_chain(3, true);
    let config = StitcherConfig::default();
    let (count, stats) = run(&graph, reference, config);
    let (limited_count, limited_stats) =
        run(&graph, reference, config.with_max_scope_depth(Some(100)));
    assert_eq!(count, limited_count);
    assert_eq!(stats.work, limited_stats.work);
}

#[test]
fn overlays_of_frozen_paths_match_scoped_resolution_on_threads() {
    let graph = test_graphs::class_field_through_function_parameter::new::<StackGraph>();
    let mut partials = PartialPaths::new();
    let db = database_for(&graph, &mut partials);
    let expected = resolve_all(&graph, &mut partials, &db, true, StitcherConfig::default());
    let before = partials.allocated_cells();

    let frozen = partials.freeze();
    let results: Vec<_> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..4)
            .map(|_| {
                let frozen = &frozen;
                let (graph, db) = (&graph, &db);
                scope.spawn(move || {
                    let mut overlay = frozen.overlay();
                    // The first new cell of an overlay follows the frozen cells.
                    assert_eq!(before, overlay.allocated_cells());
                    let first =
                        resolve_all(graph, &mut overlay, db, true, StitcherConfig::default());
                    let second =
                        resolve_all(graph, &mut overlay, db, true, StitcherConfig::default());
                    assert_eq!(before, overlay.allocated_cells());
                    assert_eq!(first, second);
                    first
                })
            })
            .collect();
        workers.into_iter().map(|w| w.join().unwrap()).collect()
    });
    for result in results {
        assert_eq!(expected, result);
    }
    let mut thawed = stack_graphs::partial::FrozenPartialPaths::thaw(frozen)
        .ok()
        .expect("no overlay is alive");
    assert_eq!(before, thawed.allocated_cells());
    let again = resolve_all(&graph, &mut thawed, &db, true, StitcherConfig::default());
    assert_eq!(expected, again);
}

#[test]
fn a_scope_in_an_overlay_rolls_back_and_leaves_the_base_unchanged() {
    let graph = test_graphs::class_field_through_function_parameter::new::<StackGraph>();
    let mut partials = PartialPaths::new();
    let db = database_for(&graph, &mut partials);
    let expected = resolve_all(&graph, &mut partials, &db, false, StitcherConfig::default());
    let before = partials.allocated_cells();

    let frozen = partials.freeze();
    let mut overlay = frozen.overlay();
    // Cells that a scope adds, and base reversals that it computes, are gone after the scope.
    let first = overlay.with_scope(|scoped| {
        let inner = resolve_all(&graph, scoped, &db, false, StitcherConfig::default());
        assert!(scoped.allocated_cells() > before);
        inner
    });
    assert_eq!(expected, first);
    assert_eq!(before, overlay.allocated_cells());

    // The scope also rolls back on a panic.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        overlay.with_scope(|scoped| {
            resolve_all(&graph, scoped, &db, false, StitcherConfig::default());
            panic!("stop")
        })
    }));
    assert!(result.is_err());
    assert_eq!(before, overlay.allocated_cells());

    // The overlay works after the rollbacks, and the base is unchanged.
    let second = resolve_all(&graph, &mut overlay, &db, false, StitcherConfig::default());
    assert_eq!(expected, second);
    drop(overlay);
    let mut thawed = stack_graphs::partial::FrozenPartialPaths::thaw(frozen)
        .ok()
        .expect("no overlay is alive");
    assert_eq!(before, thawed.allocated_cells());
    let again = resolve_all(&graph, &mut thawed, &db, false, StitcherConfig::default());
    assert_eq!(expected, again);
}
