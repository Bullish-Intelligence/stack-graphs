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

// ------------------------------------------------------------------------------------------------
// The decisions of the detector

mod detector {
    use std::cell::RefCell;
    use std::cmp::Ordering;

    use stack_graphs::arena::Handle;
    use stack_graphs::cycles::SimilarPathDetector;
    use stack_graphs::graph::Node;
    use stack_graphs::graph::StackGraph;
    use stack_graphs::partial::PartialPath;
    use stack_graphs::partial::PartialPathEdge;
    use stack_graphs::partial::PartialPaths;

    use crate::util::create_scope_node;

    /// A test bench for the detector.  A path has the number of edges as its identity: path `n`
    /// has `n` edges, and all paths start and end at one node, so they all have one key.
    struct Bench {
        graph: StackGraph,
        partials: PartialPaths,
        node: Handle<Node>,
        detector: SimilarPathDetector<PartialPath>,
        /// The identities of the paths for which the detector called `can_be_shadowed`.
        shadow_calls: RefCell<Vec<usize>>,
        /// The pairs (new, known) for which the detector called `cmp`.
        cmp_calls: RefCell<Vec<(usize, usize)>>,
    }

    impl Bench {
        fn new() -> Bench {
            let mut graph = StackGraph::new();
            let file = graph.add_file("detector").unwrap();
            let node = create_scope_node(&mut graph, file, false);
            Bench {
                graph,
                partials: PartialPaths::new(),
                node,
                detector: SimilarPathDetector::new(),
                shadow_calls: RefCell::new(Vec::new()),
                cmp_calls: RefCell::new(Vec::new()),
            }
        }

        fn path(&mut self, id: usize) -> PartialPath {
            let mut path = PartialPath::from_node(&self.graph, &mut self.partials, self.node);
            let file = self.graph.get_or_create_file("detector");
            for _ in 0..id {
                path.edges.push_back(
                    &mut self.partials,
                    PartialPathEdge {
                        source_node_id: stack_graphs::graph::NodeID::new_in_file(file, 0),
                        precedence: 0,
                    },
                );
            }
            path
        }

        /// Adds path `id`.  `shadowable` tells which paths another path can shadow, and `order`
        /// gives the order of the new path against a known path.  Returns what the detector
        /// returns: `true` if it rejects the path.
        fn add(
            &mut self,
            id: usize,
            keep_extra: bool,
            shadowable: impl Fn(usize) -> bool,
            order: impl Fn(usize, usize) -> Option<Ordering>,
        ) -> bool {
            let path = self.path(id);
            let shadow_calls = &self.shadow_calls;
            let cmp_calls = &self.cmp_calls;
            self.detector.add_path_shadow_aware(
                &self.graph,
                &mut self.partials,
                &path,
                keep_extra,
                |_, path| {
                    let id = path.edges.len();
                    shadow_calls.borrow_mut().push(id);
                    shadowable(id)
                },
                |_, new, known| {
                    let (new, known) = (new.edges.len(), known.edges.len());
                    cmp_calls.borrow_mut().push((new, known));
                    order(new, known)
                },
            )
        }
    }

    fn equal(_: usize, _: usize) -> Option<Ordering> {
        Some(Ordering::Equal)
    }

    #[test]
    fn the_first_path_is_kept_and_asks_nobody() {
        let mut bench = Bench::new();
        assert!(!bench.add(1, false, |_| true, equal));
        assert!(bench.shadow_calls.borrow().is_empty());
    }

    #[test]
    fn an_equal_path_is_dropped_if_the_known_path_cannot_be_shadowed() {
        let mut bench = Bench::new();
        assert!(!bench.add(1, false, |_| false, equal));
        assert!(bench.add(2, false, |_| false, equal));
        // Only the known path is asked.  The new path needs no answer.
        assert_eq!(vec![1], *bench.shadow_calls.borrow());
    }

    #[test]
    fn an_equal_path_is_kept_if_only_the_known_path_can_be_shadowed() {
        let mut bench = Bench::new();
        let shadowable = |id: usize| id == 1;
        assert!(!bench.add(1, false, shadowable, equal));
        // The known path can be shadowed, and the new path cannot: keep both.
        assert!(!bench.add(2, false, shadowable, equal));
        assert_eq!(vec![1, 2], *bench.shadow_calls.borrow());
        // A third path is dropped: path 2 is known, and nothing can shadow it.  The detector
        // asks nobody again, because it keeps the answers.
        assert!(bench.add(3, false, shadowable, equal));
        assert_eq!(vec![1, 2, 3], *bench.shadow_calls.borrow());
    }

    #[test]
    fn an_equal_path_is_dropped_if_both_paths_can_be_shadowed() {
        let mut bench = Bench::new();
        assert!(!bench.add(1, false, |_| true, equal));
        assert!(bench.add(2, false, |_| true, equal));
    }

    #[test]
    fn keep_extra_keeps_a_shadowable_path_while_the_bucket_holds_one_path() {
        let mut bench = Bench::new();
        assert!(!bench.add(1, true, |_| true, equal));
        // The bucket holds one path: keep the second path, and do not ask about it.
        assert!(!bench.add(2, true, |_| true, equal));
        assert_eq!(vec![1], *bench.shadow_calls.borrow());
        // The bucket holds two paths: drop the third path.
        assert!(bench.add(3, true, |_| true, equal));
    }

    #[test]
    fn keep_extra_changes_nothing_if_the_known_path_cannot_be_shadowed() {
        let mut bench = Bench::new();
        assert!(!bench.add(1, true, |_| false, equal));
        assert!(bench.add(2, true, |_| false, equal));
    }

    #[test]
    fn a_new_path_that_is_better_replaces_the_known_path() {
        let mut bench = Bench::new();
        assert!(!bench.add(1, false, |_| true, equal));
        let better = |new: usize, known: usize| {
            if new == 2 && known == 1 {
                Some(Ordering::Less)
            } else {
                panic!("the detector compares path {} with path {}", new, known)
            }
        };
        assert!(!bench.add(2, false, |_| true, better));
        assert!(bench.shadow_calls.borrow().is_empty());
        // Path 1 is gone: the new path 3 meets only path 2.
        let only_two = |new: usize, known: usize| {
            assert_eq!((3, 2), (new, known));
            Some(Ordering::Equal)
        };
        assert!(bench.add(3, false, |_| false, only_two));
    }

    #[test]
    fn a_new_path_that_is_worse_is_dropped_without_a_question() {
        let mut bench = Bench::new();
        assert!(!bench.add(1, false, |_| false, equal));
        assert!(bench.add(2, true, |_| false, |_, _| Some(Ordering::Greater)));
        assert!(bench.shadow_calls.borrow().is_empty());
    }

    #[test]
    fn a_path_that_differs_from_all_known_paths_is_kept() {
        let mut bench = Bench::new();
        assert!(!bench.add(1, false, |_| false, |_, _| None));
        assert!(!bench.add(2, false, |_| false, |_, _| None));
        assert!(bench.shadow_calls.borrow().is_empty());
        assert_eq!(vec![(2, 1)], *bench.cmp_calls.borrow());
    }

    #[test]
    fn a_path_is_kept_if_a_later_known_path_does_not_make_it_redundant() {
        // The known paths 1 and 2 are both kept.  Path 1 is equal to the new path 3 and can be
        // shadowed, and path 2 differs.  So path 3 is kept, and the detector asks about 3.
        let mut bench = Bench::new();
        let shadowable = |id: usize| id != 3;
        let order = |new: usize, known: usize| {
            if new == 2 && known == 1 {
                None
            } else if known == 1 {
                Some(Ordering::Equal)
            } else {
                None
            }
        };
        assert!(!bench.add(1, false, shadowable, order));
        assert!(!bench.add(2, false, shadowable, order));
        assert!(!bench.add(3, false, shadowable, order));
        assert_eq!(vec![1, 3], *bench.shadow_calls.borrow());
    }

    #[test]
    fn plain_add_path_drops_every_equal_path() {
        let mut bench = Bench::new();
        let path1 = bench.path(1);
        let path2 = bench.path(2);
        let cmp = |_: &mut PartialPaths, _: &PartialPath, _: &PartialPath| Some(Ordering::Equal);
        assert!(!bench
            .detector
            .add_path(&bench.graph, &mut bench.partials, &path1, cmp));
        assert!(bench
            .detector
            .add_path(&bench.graph, &mut bench.partials, &path2, cmp));
    }
}
