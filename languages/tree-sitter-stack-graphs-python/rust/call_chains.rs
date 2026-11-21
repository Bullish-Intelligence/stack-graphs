// -*- coding: utf-8 -*-
// ------------------------------------------------------------------------------------------------
// Copyright © 2024, stack-graphs authors.
// Licensed under either of Apache License, Version 2.0, or MIT license, at your option.
// Please see the LICENSE-APACHE or LICENSE-MIT files in this distribution for license details.
// ------------------------------------------------------------------------------------------------

//! Tests for the scope depth limit on the call chains of unannotated Python code.
//!
//! The rules make a reference to `x` follow the value of `x = f(...)`. The search enters `f` and
//! follows each returned expression. Each call site adds a scope to the scope stack of the path.
//! Two paths that differ only in old call sites are never similar. If each function has two
//! call sites, the number of paths doubles at each level.
//! The scope depth limit of the stitcher merges those paths.

use stack_graphs::graph::StackGraph;
use stack_graphs::partial::PartialPaths;
use stack_graphs::stitching::Database;
use stack_graphs::stitching::ForwardPartialPathStitcher;
use stack_graphs::stitching::StitchBound;
use stack_graphs::stitching::StitcherConfig;
use tree_sitter_stack_graphs::NoCancellation;
use tree_sitter_stack_graphs::Variables;

const TEST_FILE: &str = "test.py";

/// What a search for the references named `name` found.
struct Search {
    /// The zero-based lines of the definitions in the test file, sorted.
    lines: Vec<usize>,
    /// The bound that stopped a search, if any.
    bounded: Option<StitchBound>,
    /// The largest queue of a search.
    peak_queue_len: usize,
    /// The total work of the searches.
    work: usize,
}

fn search(source: &str, name: &str, config: StitcherConfig) -> Search {
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
    for f in graph.iter_files().collect::<Vec<_>>() {
        let mut paths = Vec::new();
        ForwardPartialPathStitcher::find_minimal_partial_path_set_in_file(
            &graph,
            &mut partials,
            f,
            StitcherConfig::default().with_shadow_aware_similar_paths(true),
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
        .filter(|n| {
            graph[*n]
                .symbol()
                .map(|s| &graph[s] == name)
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    assert!(!references.is_empty(), "no reference named {}", name);

    let config = config.with_shadow_aware_similar_paths(true);
    let mut result = Search {
        lines: Vec::new(),
        bounded: None,
        peak_queue_len: 0,
        work: 0,
    };
    for reference in references {
        // The search releases the paths when it ends, so the closure reads them while it runs.
        let mut kept: Vec<(stack_graphs::partial::PartialPath, usize)> = Vec::new();
        let stats = ForwardPartialPathStitcher::find_complete_partial_paths_in_scope(
            &graph,
            &mut partials,
            &db,
            reference,
            config,
            &stack_graphs::NoCancellation,
            |graph, partials, path| {
                // Drop the paths that another path shadows, as the test runner does.
                if kept.iter().any(|(other, _)| other.shadows(partials, path)) {
                    return;
                }
                kept.retain(|(other, _)| !path.shadows(partials, other));
                let end = path.end_node;
                let line = graph
                    .source_info(end)
                    .map_or(usize::MAX, |info| info.span.start.line);
                let in_test_file = graph[end].file() == Some(file);
                kept.push((path.clone(), if in_test_file { line } else { usize::MAX }));
            },
        )
        .expect("resolve");
        result.bounded = result.bounded.or(stats.bounded);
        result.peak_queue_len = result.peak_queue_len.max(stats.peak_queue_len);
        result.work += stats.work;
        for (_, line) in kept {
            if line != usize::MAX && !result.lines.contains(&line) {
                result.lines.push(line);
            }
        }
    }
    result.lines.sort();
    result
}

/// `levels` functions. Each function has two calls of the next one. The last returns `D()`.
/// Line 0 holds `class D:`, and line 1 holds the member `foo`. The reference `foo` is on the
/// last line.
fn diamond(levels: usize) -> String {
    let mut source = String::from("class D:\n    foo = 1\n\n");
    for i in 0..levels {
        if i + 1 < levels {
            source += &format!(
                "def f{i}(a):\n    if a:\n        return f{}(1)\n    return f{}(2)\n",
                i + 1,
                i + 1
            );
        } else {
            source += &format!("def f{i}(a):\n    return D()\n");
        }
    }
    source += "x = f0(0)\ny = x.foo\n";
    source
}

const QUEUE_LIMIT: usize = 2_000;

fn limited(depth: Option<usize>) -> StitcherConfig {
    StitcherConfig::default()
        .with_max_queue_len(Some(QUEUE_LIMIT))
        .with_max_work(Some(1_000_000))
        .with_max_scope_depth(depth)
}

#[test]
fn branching_call_sites_double_the_queue_at_each_level() {
    // Without a limit, the peak queue is 2^(levels + 1). These sizes stay under the queue limit.
    let mut previous = 0;
    for levels in [4, 6, 8] {
        let found = search(&diamond(levels), "foo", limited(None));
        assert_eq!(None, found.bounded, "levels {levels}");
        assert_eq!(1 << (levels + 1), found.peak_queue_len, "levels {levels}");
        assert_eq!(vec![1], found.lines);
        assert!(found.peak_queue_len > previous);
        previous = found.peak_queue_len;
    }
}

#[test]
fn a_deep_chain_hits_the_queue_limit_without_a_depth_limit() {
    let found = search(&diamond(16), "foo", limited(None));
    assert_eq!(Some(StitchBound::QueueLength), found.bounded);
}

#[test]
fn a_depth_limit_stops_the_growth_and_keeps_the_target() {
    for depth in [1, 2, 3] {
        let mut peaks = Vec::new();
        let mut works = Vec::new();
        for levels in [8, 16, 32] {
            let found = search(&diamond(levels), "foo", limited(Some(depth)));
            assert_eq!(None, found.bounded, "depth {depth} levels {levels}");
            assert_eq!(vec![1], found.lines, "depth {depth} levels {levels}");
            peaks.push(found.peak_queue_len);
            works.push(found.work);
        }
        // The queue does not grow with the levels. The work grows less than the square.
        assert_eq!(
            peaks[0],
            peaks[2],
            "{}",
            format!("depth {depth}: {peaks:?}")
        );
        assert!(
            works[2] < 8 * works[0],
            "{}",
            format!("depth {depth}: {works:?}")
        );
    }
}

/// A value that passes through two functions as a parameter needs two scopes in the stack.
const TWO_LEVELS: &str = "class D:
    foo = 1

def mk(a):
    return a

def wrap(b):
    return mk(b)

x = wrap(D())
y = x.foo
";

#[test]
fn a_depth_limit_keeps_results_that_need_few_scopes_and_loses_deeper_ones() {
    let unlimited = search(TWO_LEVELS, "foo", StitcherConfig::default());
    assert_eq!(vec![1], unlimited.lines);
    let depth_two = search(
        TWO_LEVELS,
        "foo",
        StitcherConfig::default().with_max_scope_depth(Some(2)),
    );
    assert_eq!(vec![1], depth_two.lines);
    // The limit loses a result. It never finds a different one.
    let depth_one = search(
        TWO_LEVELS,
        "foo",
        StitcherConfig::default().with_max_scope_depth(Some(1)),
    );
    assert!(depth_one.lines.is_empty());
}

/// A class with `methods` methods that store nothing. The reference `m0` is on the last line.
fn read_only_methods(methods: usize) -> String {
    let mut source = String::from("class C:\n");
    for i in 0..methods {
        source += &format!("    def m{i}(self):\n        return self.m0\n");
    }
    source += "c = C()\nz = c.m0\n";
    source
}

#[test]
fn methods_that_store_nothing_do_not_widen_a_member_lookup() {
    // The member attributes of a class lead into the methods that can store an attribute. A
    // method that stores nothing adds no route, so the queue does not grow with the methods.
    let few = search(&read_only_methods(4), "m0", StitcherConfig::default());
    let many = search(&read_only_methods(128), "m0", StitcherConfig::default());
    assert_eq!(vec![1], few.lines);
    assert_eq!(vec![1], many.lines);
    assert_eq!(few.peak_queue_len, many.peak_queue_len);
}

/// A class with `methods` methods that each store one attribute.
fn storing_methods(methods: usize) -> String {
    let mut source = String::from("class C:\n");
    for i in 0..methods {
        source += &format!("    def m{i}(self):\n        self.a{i} = 1\n");
    }
    source += "c = C()\nz = c.a0\n";
    source
}

#[test]
fn a_method_that_stores_an_attribute_still_defines_it() {
    let found = search(&storing_methods(16), "a0", StitcherConfig::default());
    assert_eq!(vec![2], found.lines);
}
