// Tests for the contraction of plain scope nodes.

use stack_graphs::graph::Degree;
use stack_graphs::graph::StackGraph;
use stack_graphs::partial::PartialPaths;
use stack_graphs::stitching::ForwardPartialPathStitcher;
use stack_graphs::stitching::StitcherConfig;
use stack_graphs::NoCancellation;

use crate::util::create_pop_symbol_node;
use crate::util::create_push_symbol_node;
use crate::util::create_scope_node;

/// Returns the (end local id, edge count) of each complete path that no other path shadows.
fn complete(
    graph: &StackGraph,
    file: stack_graphs::arena::Handle<stack_graphs::graph::File>,
) -> Vec<(u32, usize)> {
    let config = StitcherConfig::default().with_shadow_aware_similar_paths(true);
    let mut partials = PartialPaths::new();
    let mut paths = Vec::new();
    ForwardPartialPathStitcher::find_minimal_partial_path_set_in_file(
        graph,
        &mut partials,
        file,
        config,
        &NoCancellation,
        |_, _, path| paths.push(path.clone()),
    )
    .unwrap();
    let mut found = Vec::new();
    for (index, path) in paths.iter().enumerate() {
        if !path.starts_at_reference(graph) || !path.ends_at_definition(graph) {
            continue;
        }
        let shadowed = paths
            .iter()
            .enumerate()
            .any(|(other, o)| other != index && o.shadows(&mut partials, path));
        if !shadowed {
            found.push((graph[path.end_node].id().local_id(), path.edges.len()));
        }
    }
    found.sort();
    found
}

#[test]
fn a_chain_of_plain_scopes_becomes_one_edge() {
    for mode in [0u8, 1, 2] {
        let mut graph = StackGraph::new();
        let file = graph.add_file("chain").unwrap();
        let reference = create_push_symbol_node(&mut graph, file, "x", true);
        let s1 = create_scope_node(&mut graph, file, false);
        let s2 = create_scope_node(&mut graph, file, false);
        let s3 = create_scope_node(&mut graph, file, false);
        let def = create_pop_symbol_node(&mut graph, file, "x", true);
        graph.add_edge(reference, s1, 0);
        graph.add_edge(s1, s2, 0);
        graph.add_edge(s2, s3, 0);
        graph.add_edge(s3, def, 0);
        if mode > 0 {
            let stats = graph.contract_plain_scopes_in_file(file, mode);
            assert_eq!(3, stats.removable_nodes);
            assert_eq!(1, stats.redirected_edges);
        }
        let expected = if mode == 0 { 4 } else { 1 };
        assert_eq!(vec![(4, expected)], complete(&graph, file), "mode {mode}");
    }
}

#[test]
fn a_node_with_two_incoming_edges_goes_only_in_mode_two() {
    for mode in [1u8, 2] {
        let mut graph = StackGraph::new();
        let file = graph.add_file("join").unwrap();
        let r1 = create_push_symbol_node(&mut graph, file, "x", true);
        let r2 = create_push_symbol_node(&mut graph, file, "x", true);
        let join = create_scope_node(&mut graph, file, false);
        let def = create_pop_symbol_node(&mut graph, file, "x", true);
        graph.add_edge(r1, join, 0);
        graph.add_edge(r2, join, 0);
        graph.add_edge(join, def, 0);
        let stats = graph.contract_plain_scopes_in_file(file, mode);
        assert_eq!(if mode == 1 { 0 } else { 1 }, stats.removable_nodes);
        let expected = if mode == 1 { 2 } else { 1 };
        assert_eq!(
            vec![(3, expected), (3, expected)],
            complete(&graph, file),
            "mode {mode}"
        );
    }
}

#[test]
fn parallel_routes_to_one_node_do_not_merge() {
    let mut graph = StackGraph::new();
    let file = graph.add_file("diamond").unwrap();
    let reference = create_push_symbol_node(&mut graph, file, "x", true);
    let a = create_scope_node(&mut graph, file, false);
    let b = create_scope_node(&mut graph, file, false);
    let def = create_pop_symbol_node(&mut graph, file, "x", true);
    graph.add_edge(reference, a, 0);
    graph.add_edge(reference, b, 0);
    graph.add_edge(a, def, 0);
    graph.add_edge(b, def, 0);
    let before = complete(&graph, file);
    let stats = graph.contract_plain_scopes_in_file(file, 1);
    assert_eq!(1, stats.skipped_duplicates);
    let after = complete(&graph, file);
    assert_eq!(before.len(), after.len());
}

/// Builds the if-else graph of the similar path tests and returns the symbols of the reached
/// definitions after shadowing.
fn if_else(mode: u8, left_first: bool, right_binds_x: bool) -> Vec<(u32, usize)> {
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
    if mode > 0 {
        graph.contract_plain_scopes_in_file(file, mode);
    }
    complete(&graph, file)
}

#[test]
fn contraction_keeps_the_shadowing_result() {
    for left_first in [true, false] {
        for right_binds_x in [true, false] {
            let base: Vec<u32> = if_else(0, left_first, right_binds_x)
                .iter()
                .map(|x| x.0)
                .collect();
            for mode in [1u8, 2] {
                let got: Vec<u32> = if_else(mode, left_first, right_binds_x)
                    .iter()
                    .map(|x| x.0)
                    .collect();
                assert_eq!(
                    base, got,
                    "mode {mode} left_first {left_first} right_binds_x {right_binds_x}"
                );
            }
        }
    }
}

#[test]
fn a_dead_end_chain_is_pruned_and_the_result_stays() {
    let mut graph = StackGraph::new();
    let file = graph.add_file("dead").unwrap();
    let reference = create_push_symbol_node(&mut graph, file, "x", true);
    let live = create_scope_node(&mut graph, file, false);
    let dead1 = create_scope_node(&mut graph, file, false);
    let dead2 = create_scope_node(&mut graph, file, false);
    let dead3 = create_scope_node(&mut graph, file, false);
    let def = create_pop_symbol_node(&mut graph, file, "x", true);
    graph.add_edge(reference, live, 0);
    graph.add_edge(reference, dead1, 0);
    graph.add_edge(live, def, 0);
    graph.add_edge(live, dead3, 0);
    graph.add_edge(dead1, dead2, 0);
    let before = complete(&graph, file);
    let (nodes, edges) = graph.prune_dead_end_scopes_in_file(file);
    // dead1, dead2, and dead3 are dead. The edges into dead1 and dead3 go. The edge dead1 to dead2
    // goes too, because dead1 is dead.
    assert_eq!(3, nodes);
    assert_eq!(3, edges);
    assert_eq!(1, graph.outgoing_edges(reference).count());
    assert_eq!(1, graph.outgoing_edges(live).count());
    assert_eq!(0, graph.outgoing_edges(dead1).count());
    assert_eq!(before, complete(&graph, file));
}

#[test]
fn an_exported_scope_without_edges_is_not_pruned() {
    let mut graph = StackGraph::new();
    let file = graph.add_file("exported").unwrap();
    let reference = create_push_symbol_node(&mut graph, file, "x", true);
    let exported = create_scope_node(&mut graph, file, true);
    let parent = create_scope_node(&mut graph, file, false);
    graph.add_edge(reference, exported, 0);
    graph.add_edge(reference, parent, 0);
    graph.add_edge(parent, exported, 0);
    let (nodes, edges) = graph.prune_dead_end_scopes_in_file(file);
    assert_eq!((0, 0), (nodes, edges));
    assert_eq!(2, graph.outgoing_edges(reference).count());
}

#[test]
fn pruning_lets_a_chain_contract() {
    let mut graph = StackGraph::new();
    let file = graph.add_file("prune-then-contract").unwrap();
    let reference = create_push_symbol_node(&mut graph, file, "x", true);
    let a = create_scope_node(&mut graph, file, false);
    let b = create_scope_node(&mut graph, file, false);
    let dead = create_scope_node(&mut graph, file, false);
    let def = create_pop_symbol_node(&mut graph, file, "x", true);
    graph.add_edge(reference, a, 0);
    graph.add_edge(a, b, 0);
    graph.add_edge(a, dead, 0);
    graph.add_edge(b, def, 0);
    let stats = graph.simplify_file(file);
    assert_eq!(1, stats.pruned_nodes);
    assert_eq!(1, stats.pruned_edges);
    assert_eq!(2, stats.removable_nodes);
    assert_eq!(vec![(4, 1)], complete(&graph, file));
}

/// What a graph says about the nodes of one file: for each local id, the incoming degree and the
/// outgoing edges (sink local id and precedence).
fn snapshot(
    graph: &StackGraph,
    file: stack_graphs::arena::Handle<stack_graphs::graph::File>,
) -> Vec<String> {
    let mut lines = Vec::new();
    for node in graph.nodes_for_file(file) {
        let mut edges = graph
            .outgoing_edges(node)
            .map(|e| (graph[e.sink].id().local_id(), e.precedence))
            .collect::<Vec<_>>();
        edges.sort();
        lines.push(format!(
            "{}: {:?} {:?}",
            graph[node].id().local_id(),
            graph.incoming_edge_degree(node),
            edges
        ));
    }
    lines.sort();
    lines
}

/// Loads a graph into a new graph, as a storage load does, and returns the new graph and its file.
fn reload(
    graph: &StackGraph,
) -> (
    StackGraph,
    stack_graphs::arena::Handle<stack_graphs::graph::File>,
) {
    let mut loaded = StackGraph::new();
    loaded.add_from_graph(graph).unwrap();
    let file = loaded.iter_files().next().unwrap();
    (loaded, file)
}

fn assert_reload_equal(
    graph: &StackGraph,
    file: stack_graphs::arena::Handle<stack_graphs::graph::File>,
    what: &str,
) {
    let (loaded, loaded_file) = reload(graph);
    assert_eq!(
        snapshot(graph, file),
        snapshot(&loaded, loaded_file),
        "{what}"
    );
}

#[test]
fn contraction_leaves_no_edge_at_a_bypassed_node() {
    let mut graph = StackGraph::new();
    let file = graph.add_file("bypass").unwrap();
    let r = create_push_symbol_node(&mut graph, file, "x", true);
    let s = create_scope_node(&mut graph, file, false);
    let z = create_pop_symbol_node(&mut graph, file, "x", true);
    graph.add_edge(r, s, 0);
    graph.add_edge(s, z, 0);
    graph.simplify_file(file);
    assert_eq!(Degree::One, graph.incoming_edge_degree(z));
    assert_eq!(0, graph.outgoing_edges(s).count());
    assert_eq!(Degree::Zero, graph.incoming_edge_degree(s));
    let (loaded, loaded_file) = reload(&graph);
    let loaded_z = loaded
        .nodes_for_file(loaded_file)
        .find(|n| loaded[*n].is_definition())
        .unwrap();
    assert_eq!(Degree::One, loaded.incoming_edge_degree(loaded_z));
    let edges: usize = loaded
        .nodes_for_file(loaded_file)
        .map(|n| loaded.outgoing_edges(n).count())
        .sum();
    assert_eq!(1, edges);
    assert_reload_equal(&graph, file, "r -> s -> z");
}

#[test]
fn a_contracted_graph_equals_the_reloaded_graph() {
    // A chain, with a precedence on the first edge.
    let mut graph = StackGraph::new();
    let file = graph.add_file("chain").unwrap();
    let r = create_push_symbol_node(&mut graph, file, "x", true);
    let s1 = create_scope_node(&mut graph, file, false);
    let s2 = create_scope_node(&mut graph, file, false);
    let z = create_pop_symbol_node(&mut graph, file, "x", true);
    graph.add_edge(r, s1, 3);
    graph.add_edge(s1, s2, 0);
    graph.add_edge(s2, z, 0);
    graph.simplify_file(file);
    assert_reload_equal(&graph, file, "chain");

    // A diamond: one redirection is skipped, so one scope node keeps its edge.
    let mut graph = StackGraph::new();
    let file = graph.add_file("diamond").unwrap();
    let r = create_push_symbol_node(&mut graph, file, "x", true);
    let a = create_scope_node(&mut graph, file, false);
    let b = create_scope_node(&mut graph, file, false);
    let z = create_pop_symbol_node(&mut graph, file, "x", true);
    graph.add_edge(r, a, 0);
    graph.add_edge(r, b, 0);
    graph.add_edge(a, z, 0);
    graph.add_edge(b, z, 0);
    graph.simplify_file(file);
    assert_reload_equal(&graph, file, "diamond");

    // A dead-end chain next to a chain that contracts.
    let mut graph = StackGraph::new();
    let file = graph.add_file("dead").unwrap();
    let r = create_push_symbol_node(&mut graph, file, "x", true);
    let a = create_scope_node(&mut graph, file, false);
    let b = create_scope_node(&mut graph, file, false);
    let dead1 = create_scope_node(&mut graph, file, false);
    let dead2 = create_scope_node(&mut graph, file, false);
    let z = create_pop_symbol_node(&mut graph, file, "x", true);
    graph.add_edge(r, a, 0);
    graph.add_edge(a, b, 0);
    graph.add_edge(a, dead1, 0);
    graph.add_edge(dead1, dead2, 0);
    graph.add_edge(b, z, 0);
    graph.simplify_file(file);
    assert_reload_equal(&graph, file, "dead-end");
}

#[test]
fn contraction_mode_two_keeps_the_edge_of_a_node_with_another_entry() {
    let mut graph = StackGraph::new();
    let file = graph.add_file("join").unwrap();
    let r1 = create_push_symbol_node(&mut graph, file, "x", true);
    let r2 = create_push_symbol_node(&mut graph, file, "x", true);
    let s = create_scope_node(&mut graph, file, false);
    let z = create_pop_symbol_node(&mut graph, file, "x", true);
    graph.add_edge(r1, s, 0);
    graph.add_edge(r2, s, 0);
    graph.add_edge(s, z, 0);
    graph.contract_plain_scopes_in_file(file, 2);
    assert_eq!(Degree::Multiple, graph.incoming_edge_degree(z));
    assert_eq!(1, graph.outgoing_edges(s).count());
}
