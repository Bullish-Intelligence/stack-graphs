// -*- coding: utf-8 -*-
// ------------------------------------------------------------------------------------------------
// Copyright © 2021, stack-graphs authors.
// Licensed under either of Apache License, Version 2.0, or MIT license, at your option.
// Please see the LICENSE-APACHE or LICENSE-MIT files in this distribution for license details.
// ------------------------------------------------------------------------------------------------

use stack_graphs::arena::Handle;
use stack_graphs::graph::Node;
use stack_graphs::graph::NodeID;
use stack_graphs::graph::StackGraph;
use stack_graphs::partial::PartialPath;
use stack_graphs::partial::PartialPathEdgeList;
use stack_graphs::partial::PartialPaths;
use stack_graphs::partial::PartialScopeStack;
use stack_graphs::partial::PartialScopeStackBindings;
use stack_graphs::partial::PartialSymbolStackBindings;
use stack_graphs::partial::ScopeStackVariable;
use stack_graphs::partial::SymbolStackVariable;
use stack_graphs::paths::PathResolutionError;
use stack_graphs::stitching::Database;

use crate::util::*;

#[test]
fn will_skip_divergent_partial_paths() {
    let mut graph = StackGraph::new();
    let mut partials = PartialPaths::new();
    let mut db = Database::new();
    let start_node = StackGraph::root_node();
    let end_node = StackGraph::root_node();
    let symbol_stack_precondition = create_symbol_stack(&mut graph, &mut partials, (&[], None));
    let symbol_stack_postcondition =
        create_symbol_stack(&mut graph, &mut partials, (&[("a", None)], None));
    let variable = ScopeStackVariable::new(1).unwrap();
    let scope_stack_precondition = PartialScopeStack::from_variable(variable);
    let scope_stack_postcondition = PartialScopeStack::from_variable(variable);
    let edges = PartialPathEdgeList::empty();
    let partial_path = PartialPath {
        start_node,
        end_node,
        symbol_stack_precondition,
        symbol_stack_postcondition,
        scope_stack_precondition,
        scope_stack_postcondition,
        edges,
    };
    db.add_partial_path(&graph, &mut partials, partial_path);
}

#[test]
fn can_apply_offset_to_partial_symbol_stacks() {
    fn verify(
        stack: NiceSymbolStack,
        symbol_variable_offset: u32,
        scope_variable_offset: u32,
        expected: &str,
    ) {
        let mut graph = StackGraph::new();
        let mut partials = PartialPaths::new();
        let stack = create_symbol_stack(&mut graph, &mut partials, stack);
        let with_offset =
            stack.with_offset(&mut partials, symbol_variable_offset, scope_variable_offset);
        let actual = with_offset.display(&graph, &mut partials).to_string();
        assert_eq!(expected, actual);
    }

    verify((&[], None), 0, 0, "");
    verify((&[], None), 0, 1, "");
    verify((&[], None), 1, 0, "");
    verify((&[], None), 1, 1, "");

    let a = ("a", None);
    verify((&[a], None), 0, 0, "a");
    verify((&[a], None), 0, 1, "a");
    verify((&[a], None), 1, 0, "a");
    verify((&[a], None), 1, 1, "a");

    let var1 = Some(SymbolStackVariable::new(1).unwrap());
    verify((&[a], var1), 0, 0, "a,%1");
    verify((&[a], var1), 0, 1, "a,%1");
    verify((&[a], var1), 1, 0, "a,%2");
    verify((&[a], var1), 1, 1, "a,%2");

    let empty_scopes: NiceScopeStack = (&[], None);
    let a_empty = ("a", Some(empty_scopes));
    verify((&[a_empty], None), 0, 0, "a/()");
    verify((&[a_empty], None), 0, 1, "a/()");
    verify((&[a_empty], None), 1, 0, "a/()");
    verify((&[a_empty], None), 1, 1, "a/()");
    verify((&[a_empty], var1), 0, 0, "a/(),%1");
    verify((&[a_empty], var1), 0, 1, "a/(),%1");
    verify((&[a_empty], var1), 1, 0, "a/(),%2");
    verify((&[a_empty], var1), 1, 1, "a/(),%2");

    let scope_var1 = Some(ScopeStackVariable::new(1).unwrap());
    let scopes_var1: NiceScopeStack = (&[], scope_var1);
    let a_var1 = ("a", Some(scopes_var1));
    verify((&[a_var1], None), 0, 0, "a/($1)");
    verify((&[a_var1], None), 0, 1, "a/($2)");
    verify((&[a_var1], None), 1, 0, "a/($1)");
    verify((&[a_var1], None), 1, 1, "a/($2)");
    verify((&[a_var1], var1), 0, 0, "a/($1),%1");
    verify((&[a_var1], var1), 0, 1, "a/($2),%1");
    verify((&[a_var1], var1), 1, 0, "a/($1),%2");
    verify((&[a_var1], var1), 1, 1, "a/($2),%2");
}

#[test]
fn can_unify_partial_symbol_stacks() -> Result<(), PathResolutionError> {
    fn verify(
        lhs: NiceSymbolStack,
        rhs: NiceSymbolStack,
        expected_unification: &str,
        expected_symbol_bindings: &str,
        expected_scope_bindings: &str,
    ) -> Result<(), PathResolutionError> {
        let mut graph = StackGraph::new();
        let mut partials = PartialPaths::new();
        let lhs = create_symbol_stack(&mut graph, &mut partials, lhs);
        let rhs = create_symbol_stack(&mut graph, &mut partials, rhs);
        let mut symbol_bindings = PartialSymbolStackBindings::new();
        let mut scope_bindings = PartialScopeStackBindings::new();
        let unified = lhs.unify(
            &mut partials,
            rhs,
            &mut symbol_bindings,
            &mut scope_bindings,
        )?;
        let unified = unified.display(&graph, &mut partials).to_string();
        assert_eq!(expected_unification, unified);
        let symbol_bindings = symbol_bindings.display(&graph, &mut partials).to_string();
        assert_eq!(expected_symbol_bindings, symbol_bindings);
        let scope_bindings = scope_bindings.display(&graph, &mut partials).to_string();
        assert_eq!(expected_scope_bindings, scope_bindings);
        Ok(())
    }

    fn verify_not(lhs: NiceSymbolStack, rhs: NiceSymbolStack) -> Result<(), PathResolutionError> {
        let mut graph = StackGraph::new();
        let mut partials = PartialPaths::new();
        let lhs = create_symbol_stack(&mut graph, &mut partials, lhs);
        let rhs = create_symbol_stack(&mut graph, &mut partials, rhs);
        let mut symbol_bindings = PartialSymbolStackBindings::new();
        let mut scope_bindings = PartialScopeStackBindings::new();
        assert!(lhs
            .unify(
                &mut partials,
                rhs,
                &mut symbol_bindings,
                &mut scope_bindings
            )
            .is_err());
        Ok(())
    }

    let var1 = Some(SymbolStackVariable::new(1).unwrap());
    let var2 = Some(SymbolStackVariable::new(2).unwrap());
    let a = ("a", None);

    verify((&[], None), (&[], None), "", "{}", "{}")?;
    verify((&[], var1), (&[], None), "%1", "{%1 => <>}", "{}")?;
    verify((&[], None), (&[], var2), "%2", "{%2 => <>}", "{}")?;
    verify((&[], var1), (&[], var2), "%1", "{%2 => <%1>}", "{}")?;

    verify_not(
        (&[], None), //
        (&[a], None),
    )?;
    verify(
        (&[], var1), //
        (&[a], None),
        "a",
        "{%1 => <a>}",
        "{}",
    )?;
    verify_not(
        (&[], None), //
        (&[a], var2),
    )?;
    verify(
        (&[], var1), //
        (&[a], var2),
        "a,%2",
        "{%1 => <a,%2>}",
        "{}",
    )?;

    verify_not(
        (&[a], None), //
        (&[], None),
    )?;
    verify_not(
        (&[a], var1), //
        (&[], None),
    )?;
    verify(
        (&[a], None), //
        (&[], var2),
        "a",
        "{%2 => <a>}",
        "{}",
    )?;
    verify(
        (&[a], var1), //
        (&[], var2),
        "a,%1",
        "{%2 => <a,%1>}",
        "{}",
    )?;

    verify(
        (&[a], None), //
        (&[a], None),
        "a",
        "{}",
        "{}",
    )?;
    verify(
        (&[a], var1), //
        (&[a], None),
        "a,%1",
        "{%1 => <>}",
        "{}",
    )?;
    verify(
        (&[a], None), //
        (&[a], var2),
        "a,%2",
        "{%2 => <>}",
        "{}",
    )?;
    verify(
        (&[a], var1), //
        (&[a], var2),
        "a,%1",
        "{%2 => <%1>}",
        "{}",
    )?;

    verify_not((&[a], var1), (&[], var1))?;
    verify_not((&[], var1), (&[a], var1))?;

    let dot = (".", None);
    let b = ("b", None);

    verify(
        (&[a, dot, b], None),
        (&[a, dot, b], None),
        "a.b",
        "{}",
        "{}",
    )?;
    verify(
        (&[a, dot, b], var1),
        (&[a, dot, b], None),
        "a.b,%1",
        "{%1 => <>}",
        "{}",
    )?;
    verify(
        (&[a, dot, b], None),
        (&[a, dot, b], var2),
        "a.b,%2",
        "{%2 => <>}",
        "{}",
    )?;
    verify(
        (&[a, dot, b], var1),
        (&[a, dot, b], var2),
        "a.b,%1",
        "{%2 => <%1>}",
        "{}",
    )?;

    verify_not(
        (&[a], None), //
        (&[a, dot, b], None),
    )?;
    verify_not(
        (&[a], None), //
        (&[a, dot, b], var2),
    )?;
    verify(
        (&[a], var1), //
        (&[a, dot, b], None),
        "a.b",
        "{%1 => <.b>}",
        "{}",
    )?;
    verify(
        (&[a], var1), //
        (&[a, dot, b], var2),
        "a.b,%2",
        "{%1 => <.b,%2>}",
        "{}",
    )?;

    let empty_scopes: NiceScopeStack = (&[], None);
    let a_empty = ("a", Some(empty_scopes));

    let scope_var1 = Some(ScopeStackVariable::new(1).unwrap());
    let scopes_var1: NiceScopeStack = (&[], scope_var1);
    let a_var1 = ("a", Some(scopes_var1));

    verify_not((&[a], None), (&[a_empty], None))?;
    verify_not((&[a_empty], None), (&[a], None))?;

    verify_not((&[a], None), (&[a_var1], None))?;
    verify_not((&[a_var1], None), (&[a], None))?;

    verify(
        (&[a_empty], None),
        (&[a_var1], None),
        "a/($1)",
        "{}",
        "{$1 => ()}",
    )?;
    verify(
        (&[a_var1], None),
        (&[a_empty], None),
        "a/($1)",
        "{}",
        "{$1 => ()}",
    )?;

    Ok(())
}

#[test]
fn can_unify_partial_scope_stacks() -> Result<(), PathResolutionError> {
    fn verify(
        lhs: NiceScopeStack,
        rhs: NiceScopeStack,
        expected_unification: &str,
        expected_bindings: &str,
    ) -> Result<(), PathResolutionError> {
        let mut graph = StackGraph::new();
        let mut partials = PartialPaths::new();
        let lhs = create_scope_stack(&mut graph, &mut partials, lhs);
        let rhs = create_scope_stack(&mut graph, &mut partials, rhs);
        let mut bindings = PartialScopeStackBindings::new();
        let unified = lhs.unify(&mut partials, rhs, &mut bindings)?;
        let unified = unified.display(&graph, &mut partials).to_string();
        assert_eq!(expected_unification, unified);
        let bindings = bindings.display(&graph, &mut partials).to_string();
        assert_eq!(expected_bindings, bindings);
        Ok(())
    }

    fn verify_not(lhs: NiceScopeStack, rhs: NiceScopeStack) -> Result<(), PathResolutionError> {
        let mut graph = StackGraph::new();
        let mut partials = PartialPaths::new();
        let lhs = create_scope_stack(&mut graph, &mut partials, lhs);
        let rhs = create_scope_stack(&mut graph, &mut partials, rhs);
        let mut bindings = PartialScopeStackBindings::new();
        assert!(lhs.unify(&mut partials, rhs, &mut bindings).is_err());
        Ok(())
    }

    let var1 = Some(ScopeStackVariable::new(1).unwrap());
    let var2 = Some(ScopeStackVariable::new(2).unwrap());

    verify((&[], None), (&[], None), "", "{}")?;
    verify((&[], var1), (&[], None), "$1", "{$1 => ()}")?;
    verify((&[], None), (&[], var2), "$2", "{$2 => ()}")?;
    verify((&[], var1), (&[], var2), "$1", "{$2 => ($1)}")?;

    verify_not(
        (&[], None), //
        (&[10], None),
    )?;
    verify(
        (&[], var1), //
        (&[10], None),
        "[file(10)]",
        "{$1 => ([file(10)])}",
    )?;
    verify_not(
        (&[], None), //
        (&[10], var2),
    )?;
    verify(
        (&[], var1), //
        (&[10], var2),
        "[file(10)],$2",
        "{$1 => ([file(10)],$2)}",
    )?;

    verify_not(
        (&[10], None), //
        (&[], None),
    )?;
    verify_not(
        (&[10], var1), //
        (&[], None),
    )?;
    verify(
        (&[10], None), //
        (&[], var2),
        "[file(10)]",
        "{$2 => ([file(10)])}",
    )?;
    verify(
        (&[10], var1), //
        (&[], var2),
        "[file(10)],$1",
        "{$2 => ([file(10)],$1)}",
    )?;

    verify(
        (&[10], None), //
        (&[10], None),
        "[file(10)]",
        "{}",
    )?;
    verify(
        (&[10], var1), //
        (&[10], None),
        "[file(10)],$1",
        "{$1 => ()}",
    )?;
    verify(
        (&[10], None), //
        (&[10], var2),
        "[file(10)],$2",
        "{$2 => ()}",
    )?;
    verify(
        (&[10], var1), //
        (&[10], var2),
        "[file(10)],$1",
        "{$2 => ($1)}",
    )?;

    verify_not((&[10], var1), (&[], var1))?;
    verify_not((&[], var1), (&[10], var1))?;

    Ok(())
}

#[test]
fn can_create_partial_path_from_node() {
    let mut graph = StackGraph::new();
    let file = graph.add_file("test").expect("");
    let foo = graph.add_symbol("foo");
    let drop_scopes_node = graph
        .add_drop_scopes_node(NodeID::new_in_file(file, 0))
        .unwrap();
    let jump_to_scope_node = StackGraph::jump_to_node();
    let pop_symbol_node = graph
        .add_pop_symbol_node(NodeID::new_in_file(file, 1), foo, false)
        .unwrap();
    let pop_scoped_symbol_node = graph
        .add_pop_scoped_symbol_node(NodeID::new_in_file(file, 2), foo, false)
        .unwrap();
    let push_symbol_node = graph
        .add_push_symbol_node(NodeID::new_in_file(file, 3), foo, false)
        .unwrap();
    let exported_scope_id = NodeID::new_in_file(file, 99);
    graph.add_scope_node(exported_scope_id, true);
    let push_scoped_symbol_node = graph
        .add_push_scoped_symbol_node(NodeID::new_in_file(file, 4), foo, exported_scope_id, false)
        .unwrap();
    let root_node = StackGraph::root_node();
    let scope_node = graph
        .add_scope_node(NodeID::new_in_file(file, 5), false)
        .unwrap();

    fn verify(graph: &StackGraph, node: Handle<Node>, expected: &str) {
        let mut partials = PartialPaths::new();
        let path = PartialPath::from_node(graph, &mut partials, node);
        let actual = path.display(&graph, &mut partials).to_string();
        assert_eq!(expected, actual);
    }

    verify(
        &graph,
        drop_scopes_node,
        "<%1> ($1) [test(0) drop scopes] -> [test(0) drop scopes] <%1> ()",
    );

    verify(
        &graph,
        jump_to_scope_node,
        "<%1> ($1) [jump to scope] -> [jump to scope] <%1> ($1)",
    );

    verify(
        &graph,
        pop_symbol_node,
        "<foo,%1> ($1) [test(1) pop foo] -> [test(1) pop foo] <%1> ($1)",
    );

    verify(
        &graph,
        pop_scoped_symbol_node,
        "<foo/($2),%1> ($1) [test(2) pop scoped foo] -> [test(2) pop scoped foo] <%1> ($2)",
    );

    verify(
        &graph,
        push_symbol_node,
        "<%1> ($1) [test(3) push foo] -> [test(3) push foo] <foo,%1> ($1)",
    );

    verify(
        &graph,
        push_scoped_symbol_node,
        "<%1> ($1) [test(4) push scoped foo test(99)] -> [test(4) push scoped foo test(99)] <foo/([test(99)],$1),%1> ($1)",
    );

    verify(&graph, root_node, "<%1> ($1) [root] -> [root] <%1> ($1)");

    verify(
        &graph,
        scope_node,
        "<%1> ($1) [test(5) scope] -> [test(5) scope] <%1> ($1)",
    );
}

#[test]
fn can_append_edges() -> Result<(), PathResolutionError> {
    let mut graph = StackGraph::new();
    let file = graph.add_file("test").expect("");
    let jump_to_scope_node = StackGraph::jump_to_node();
    let scope0 = create_scope_node(&mut graph, file, false);
    let scope1 = create_scope_node(&mut graph, file, false);
    let foo_ref = create_push_symbol_node(&mut graph, file, "foo", false);
    let foo_def = create_pop_symbol_node(&mut graph, file, "foo", false);
    let bar_ref = create_push_symbol_node(&mut graph, file, "bar", false);
    let bar_def = create_pop_symbol_node(&mut graph, file, "bar", false);
    let exported_scope = create_scope_node(&mut graph, file, true);
    let exported_scope_id = graph[exported_scope].id();
    let baz_ref = create_push_scoped_symbol_node(&mut graph, file, "baz", exported_scope_id, false);
    let baz_def = create_pop_scoped_symbol_node(&mut graph, file, "baz", false);
    let drop_scopes = create_drop_scopes_node(&mut graph, file);

    fn run(
        graph: &StackGraph,
        left: NicePartialPath,
        right: NiceEdge,
        expected: &str,
    ) -> Result<(), PathResolutionError> {
        let mut g = StackGraph::new();
        g.add_from_graph(graph).expect("");

        let mut ps = PartialPaths::new();

        let mut l = create_partial_path_and_edges(&mut g, &mut ps, left).expect("");
        let r = create_edge(&mut g, right);

        l.append(graph, &mut ps, r)?;
        let actual = l.display(&g, &mut ps).to_string();
        assert_eq!(expected, actual);

        Ok(())
    }

    fn verify(graph: &StackGraph, left: NicePartialPath, right: NiceEdge, expected: &str) {
        run(graph, left, right, expected).expect("");
    }

    fn verify_not(graph: &StackGraph, left: NicePartialPath, right: NiceEdge) {
        run(graph, left, right, "").expect_err("");
    }

    verify(
        &graph,
        &[foo_ref, scope0],
        (scope0, foo_def),
        "<%1> ($1) [test(2) push foo] -> [test(3) pop foo] <%1> ($1)",
    );

    verify(
        &graph,
        &[foo_ref, scope0],
        (scope0, scope1),
        "<%1> ($1) [test(2) push foo] -> [test(1) scope] <foo,%1> ($1)",
    );

    verify(
        &graph,
        &[scope0, scope1],
        (scope1, foo_ref),
        "<%1> ($1) [test(0) scope] -> [test(2) push foo] <foo,%1> ($1)",
    );

    verify(
        &graph,
        &[foo_def, scope0],
        (scope0, bar_ref),
        "<foo,%1> ($1) [test(3) pop foo] -> [test(4) push bar] <bar,%1> ($1)",
    );

    verify(
        &graph,
        &[foo_ref, scope0, foo_def],
        (foo_def, bar_ref),
        "<%1> ($1) [test(2) push foo] -> [test(4) push bar] <bar,%1> ($1)",
    );

    verify(
        &graph,
        &[foo_def, scope0, bar_ref],
        (bar_ref, bar_def),
        "<foo,%1> ($1) [test(3) pop foo] -> [test(5) pop bar] <%1> ($1)",
    );

    verify(
        &graph,
        &[baz_ref, scope0, baz_def],
        (baz_def, bar_ref),
        "<%1> ($1) [test(7) push scoped baz test(6)] -> [test(4) push bar] <bar,%1> ([test(6)],$1)",
    );

    verify(
        &graph,
        &[foo_def, scope0, baz_ref],
        (baz_ref, baz_def),
        "<foo,%1> ($1) [test(3) pop foo] -> [test(8) pop scoped baz] <%1> ([test(6)],$1)",
    );

    verify(
        &graph,
        &[scope0, drop_scopes],
        (drop_scopes, scope1),
        "<%1> ($1) [test(0) scope] -> [test(1) scope] <%1> ()",
    );

    verify_not(
        &graph,
        &[scope0, drop_scopes, scope1],
        (scope1, jump_to_scope_node),
    );

    verify(
        &graph,
        &[baz_def, scope0],
        (scope0, jump_to_scope_node),
        "<baz/($2),%1> ($1) [test(8) pop scoped baz] -> [jump to scope] <%1> ($2)",
    );

    verify(
        &graph,
        &[baz_ref, scope0],
        (scope0, jump_to_scope_node),
        "<%1> ($1) [test(7) push scoped baz test(6)] -> [jump to scope] <baz/([test(6)],$1),%1> ($1)",
    );

    Ok(())
}

#[test]
fn can_append_edges_without_precondition_variables() -> Result<(), PathResolutionError> {
    let mut graph = StackGraph::new();
    let file = graph.add_file("test").expect("");
    let scope0 = create_scope_node(&mut graph, file, false);
    let foo_ref = create_push_symbol_node(&mut graph, file, "foo", false);
    let foo_def = create_pop_symbol_node(&mut graph, file, "foo", false);
    let bar_ref = create_push_symbol_node(&mut graph, file, "bar", false);
    let bar_def = create_pop_symbol_node(&mut graph, file, "bar", false);

    fn run(
        graph: &StackGraph,
        left: NicePartialPath,
        right: NiceEdge,
        expected: &str,
    ) -> Result<(), PathResolutionError> {
        let mut g = StackGraph::new();
        g.add_from_graph(graph).expect("");

        let mut ps = PartialPaths::new();

        let mut l = create_partial_path_and_edges(&mut g, &mut ps, left).expect("");
        l.eliminate_precondition_stack_variables(&mut ps);
        let r = create_edge(&mut g, right);

        l.append(graph, &mut ps, r)?;
        let actual = l.display(&g, &mut ps).to_string();
        assert_eq!(expected, actual);

        Ok(())
    }

    fn verify(graph: &StackGraph, left: NicePartialPath, right: NiceEdge, expected: &str) {
        run(graph, left, right, expected).expect("");
    }

    fn verify_not(graph: &StackGraph, left: NicePartialPath, right: NiceEdge) {
        run(graph, left, right, "").expect_err("");
    }

    verify(
        &graph,
        &[foo_ref, scope0, foo_def],
        (foo_def, bar_ref),
        "<> () [test(1) push foo] -> [test(3) push bar] <bar> ()",
    );

    verify_not(&graph, &[scope0], (scope0, bar_def));

    verify_not(&graph, &[foo_ref, scope0, foo_def], (foo_def, bar_def));

    Ok(())
}

#[test]
fn can_append_partial_paths() -> Result<(), PathResolutionError> {
    let mut graph = StackGraph::new();
    let file = graph.add_file("test").expect("");
    let jump_to_scope_node = StackGraph::jump_to_node();
    let scope0 = create_scope_node(&mut graph, file, false);
    let scope1 = create_scope_node(&mut graph, file, false);
    let foo_ref = create_push_symbol_node(&mut graph, file, "foo", false);
    let foo_def = create_pop_symbol_node(&mut graph, file, "foo", false);
    let bar_ref = create_push_symbol_node(&mut graph, file, "bar", false);
    let bar_def = create_pop_symbol_node(&mut graph, file, "bar", false);
    let exported_scope = create_scope_node(&mut graph, file, true);
    let exported_scope_id = graph[exported_scope].id();
    let baz_ref = create_push_scoped_symbol_node(&mut graph, file, "baz", exported_scope_id, false);
    let baz_def = create_pop_scoped_symbol_node(&mut graph, file, "baz", false);
    let drop_scopes = create_drop_scopes_node(&mut graph, file);

    fn run(
        graph: &StackGraph,
        left: NicePartialPath,
        right: NicePartialPath,
        expected: &str,
    ) -> Result<(), PathResolutionError> {
        let mut g = StackGraph::new();
        g.add_from_graph(graph).expect("");

        let mut ps = PartialPaths::new();

        let mut l = create_partial_path_and_edges(&mut g, &mut ps, left).expect("");
        let mut r = create_partial_path_and_edges(&mut g, &mut ps, right).expect("");

        r.ensure_no_overlapping_variables(&mut ps, &l);
        l.concatenate(&g, &mut ps, &r)?;
        let actual = l.display(&g, &mut ps).to_string();
        assert_eq!(expected, actual);

        Ok(())
    }

    fn verify(graph: &StackGraph, left: NicePartialPath, right: NicePartialPath, expected: &str) {
        run(graph, left, right, expected).expect("");
    }

    fn verify_not(graph: &StackGraph, left: NicePartialPath, right: NicePartialPath) {
        run(graph, left, right, "").expect_err("");
    }

    verify(
        &graph,
        &[scope0],
        &[scope0],
        "<%1> ($1) [test(0) scope] -> [test(0) scope] <%1> ($1)",
    );

    verify_not(&graph, &[scope0], &[scope1]);

    verify(
        &graph,
        &[foo_ref, scope0],
        &[scope0, foo_def],
        "<%1> ($1) [test(2) push foo] -> [test(3) pop foo] <%1> ($1)",
    );

    verify(
        &graph,
        &[foo_ref, scope0],
        &[scope0, scope1],
        "<%1> ($1) [test(2) push foo] -> [test(1) scope] <foo,%1> ($1)",
    );

    verify(
        &graph,
        &[scope0, scope1],
        &[scope1, foo_ref],
        "<%1> ($1) [test(0) scope] -> [test(2) push foo] <foo,%1> ($1)",
    );

    verify(
        &graph,
        &[foo_def, scope0],
        &[scope0, bar_ref],
        "<foo,%1> ($1) [test(3) pop foo] -> [test(4) push bar] <bar,%1> ($1)",
    );

    verify(
        &graph,
        &[foo_ref, scope0, foo_def],
        &[foo_def, scope1, bar_ref],
        "<%1> ($1) [test(2) push foo] -> [test(4) push bar] <bar,%1> ($1)",
    );

    verify(
        &graph,
        &[foo_def, scope0, bar_ref],
        &[bar_ref, scope1, bar_def],
        "<foo,%1> ($1) [test(3) pop foo] -> [test(5) pop bar] <%1> ($1)",
    );

    verify(
        &graph,
        &[baz_ref, scope0, baz_def],
        &[baz_def, scope1, bar_ref],
        "<%1> ($1) [test(7) push scoped baz test(6)] -> [test(4) push bar] <bar,%1> ([test(6)],$1)",
    );

    verify(
        &graph,
        &[foo_def, scope0, baz_ref],
        &[baz_ref, scope1, baz_def],
        "<foo,%1> ($1) [test(3) pop foo] -> [test(8) pop scoped baz] <%1> ([test(6)],$1)",
    );

    verify(
        &graph,
        &[scope0, drop_scopes],
        &[drop_scopes, scope1],
        "<%1> ($1) [test(0) scope] -> [test(1) scope] <%1> ()",
    );

    verify_not(
        &graph,
        &[scope0, drop_scopes, scope1],
        &[scope1, jump_to_scope_node],
    );

    verify(
        &graph,
        &[baz_def, scope0],
        &[scope0, jump_to_scope_node],
        "<baz/($2),%1> ($1) [test(8) pop scoped baz] -> [jump to scope] <%1> ($2)",
    );

    verify(
        &graph,
        &[baz_ref, scope0],
        &[scope0, jump_to_scope_node],
        "<%1> ($1) [test(7) push scoped baz test(6)] -> [jump to scope] <baz/([test(6)],$1),%1> ($1)",
    );

    Ok(())
}

#[test]
fn can_append_partial_paths_without_precondition_variables() -> Result<(), PathResolutionError> {
    let mut graph = StackGraph::new();
    let file = graph.add_file("test").expect("");
    let scope0 = create_scope_node(&mut graph, file, false);
    let foo_ref = create_push_symbol_node(&mut graph, file, "foo", false);
    let foo_def = create_pop_symbol_node(&mut graph, file, "foo", false);
    let bar_ref = create_push_symbol_node(&mut graph, file, "bar", false);
    let bar_def = create_pop_symbol_node(&mut graph, file, "bar", false);

    fn run(
        graph: &StackGraph,
        left: NicePartialPath,
        right: NicePartialPath,
        expected: &str,
    ) -> Result<(), PathResolutionError> {
        let mut g = StackGraph::new();
        g.add_from_graph(graph).expect("");

        let mut ps = PartialPaths::new();

        let mut l = create_partial_path_and_edges(&mut g, &mut ps, left).expect("");
        l.eliminate_precondition_stack_variables(&mut ps);
        let mut r = create_partial_path_and_edges(&mut g, &mut ps, right).expect("");

        r.ensure_no_overlapping_variables(&mut ps, &l);
        l.concatenate(&g, &mut ps, &r)?;
        let actual = l.display(&g, &mut ps).to_string();
        assert_eq!(expected, actual);

        Ok(())
    }

    fn verify(graph: &StackGraph, left: NicePartialPath, right: NicePartialPath, expected: &str) {
        run(graph, left, right, expected).expect("");
    }

    fn verify_not(graph: &StackGraph, left: NicePartialPath, right: NicePartialPath) {
        run(graph, left, right, "").expect_err("");
    }

    verify(
        &graph,
        &[foo_ref, scope0],
        &[scope0, foo_def, bar_ref],
        "<> () [test(1) push foo] -> [test(3) push bar] <bar> ()",
    );

    verify_not(&graph, &[scope0], &[scope0, bar_def]);

    verify_not(&graph, &[foo_ref, scope0, foo_def], &[foo_def, bar_def]);

    Ok(())
}

#[test]
fn can_resolve_to_node() -> Result<(), PathResolutionError> {
    let mut graph = StackGraph::new();
    let file = graph.add_file("test").expect("");

    let jump_to_scope_node = StackGraph::jump_to_node();
    let exported_scope = create_scope_node(&mut graph, file, true);
    let baz_def = create_pop_scoped_symbol_node(&mut graph, file, "baz", false);

    // resolved node ends up in precondition scope stack
    {
        let mut g = StackGraph::new();
        g.add_from_graph(&graph).expect("");

        let mut ps = PartialPaths::new();
        let mut p =
            create_partial_path_and_edges(&mut g, &mut ps, &[jump_to_scope_node]).expect("");

        p.resolve_to_node(&g, &mut ps, exported_scope).expect("");
        assert_eq!(
            Some(exported_scope),
            p.scope_stack_precondition.pop_front(&mut ps)
        );
    }

    // precondition is fixed, and resolving fails
    {
        let mut g = StackGraph::new();
        g.add_from_graph(&graph).expect("");

        let mut ps = PartialPaths::new();
        let mut p =
            create_partial_path_and_edges(&mut g, &mut ps, &[jump_to_scope_node]).expect("");

        p.eliminate_precondition_stack_variables(&mut ps);
        p.resolve_to_node(&g, &mut ps, exported_scope)
            .expect_err("");
    }

    // resolved node ends up in scoped symbol in precondition symbol stack
    // resolving succeeds even though the scope stack is finite
    {
        let mut g = StackGraph::new();
        g.add_from_graph(&graph).expect("");

        let mut ps = PartialPaths::new();
        let mut p = create_partial_path_and_edges(&mut g, &mut ps, &[baz_def, jump_to_scope_node])
            .expect("");

        p.eliminate_precondition_stack_variables(&mut ps);
        p.resolve_to_node(&g, &mut ps, exported_scope).expect("");
        assert_eq!(
            Some(exported_scope),
            p.symbol_stack_precondition
                .pop_front(&mut ps)
                .and_then(|s| s.scopes.into_option())
                .and_then(|mut st| st.pop_front(&mut ps))
        );
    }

    Ok(())
}

/// The reference definition of shadowing between two edge lists.  The first position where both
/// lists have an edge decides: different nodes mean no shadowing, and a higher precedence of the
/// same node means shadowing.
fn expected_shadows(left: &[(u32, i32)], right: &[(u32, i32)]) -> bool {
    for (l, r) in left.iter().zip(right.iter()) {
        if l.0 != r.0 {
            return false;
        }
        if l.1 > r.1 {
            return true;
        }
    }
    false
}

fn edge_list(
    graph: &mut StackGraph,
    partials: &mut PartialPaths,
    edges: &[(u32, i32)],
    forwards: bool,
) -> PartialPathEdgeList {
    let file = graph.get_or_create_file("edges");
    let mut list = PartialPathEdgeList::empty();
    let edge = |(id, precedence): (u32, i32)| stack_graphs::partial::PartialPathEdge {
        source_node_id: NodeID::new_in_file(file, id),
        precedence,
    };
    if forwards {
        for e in edges.iter().rev() {
            list.push_front(partials, edge(*e));
        }
    } else {
        for e in edges {
            list.push_back(partials, edge(*e));
        }
    }
    list
}

/// Makes the lists of 0 to 6 edges, from three nodes and three precedences.
fn all_edge_sequences() -> Vec<Vec<(u32, i32)>> {
    let mut all = vec![vec![]];
    let mut previous = vec![vec![]];
    for _ in 0..4 {
        let mut next = Vec::new();
        for sequence in &previous {
            for node in 1..=2u32 {
                for precedence in 0..=2i32 {
                    let mut longer = sequence.clone();
                    longer.push((node, precedence));
                    next.push(longer);
                }
            }
        }
        all.extend(next.iter().cloned());
        previous = next;
    }
    all
}

#[test]
fn edge_lists_shadow_as_the_reference_says_in_every_direction() {
    let mut graph = StackGraph::new();
    let mut partials = PartialPaths::new();
    let sequences = all_edge_sequences();
    for left in sequences.iter().step_by(7) {
        for right in sequences.iter().step_by(5) {
            let expected = expected_shadows(left, right);
            for (left_forwards, right_forwards) in
                [(false, false), (true, false), (false, true), (true, true)]
            {
                let l = edge_list(&mut graph, &mut partials, left, left_forwards);
                let r = edge_list(&mut graph, &mut partials, right, right_forwards);
                assert_eq!(
                    expected,
                    l.shadows(&mut partials, r),
                    "{:?} shadows {:?}, forwards: {} {}",
                    left,
                    right,
                    left_forwards,
                    right_forwards
                );
            }
        }
    }
}

#[test]
fn shadowing_of_edge_lists_that_the_stitcher_builds_needs_no_new_cells() {
    let mut graph = StackGraph::new();
    let mut partials = PartialPaths::new();
    let left = edge_list(&mut graph, &mut partials, &[(1, 0), (2, 0), (1, 1), (2, 2)], false);
    let right = edge_list(&mut graph, &mut partials, &[(1, 0), (2, 0), (1, 0), (2, 0)], false);
    let cells = partials.allocated_cells();
    assert!(left.shadows(&mut partials, right));
    assert!(!right.shadows(&mut partials, left));
    assert_eq!(cells, partials.allocated_cells());
}

#[test]
fn applying_bindings_to_a_stack_that_the_stitcher_builds_needs_no_reversal() {
    use controlled_option::ControlledOption;
    use stack_graphs::partial::PartialScopedSymbol;
    use stack_graphs::partial::PartialSymbolStack;

    let mut graph = StackGraph::new();
    let mut partials = PartialPaths::new();
    // A path stitcher pushes the symbols of a postcondition onto the front.
    let mut forwards = PartialSymbolStack::empty();
    for name in ["c", "b", "a"] {
        let symbol = graph.add_symbol(name);
        let scoped = PartialScopedSymbol {
            symbol,
            scopes: ControlledOption::none(),
        };
        forwards.push_front(&mut partials, scoped);
    }
    let backwards = create_symbol_stack(
        &mut graph,
        &mut partials,
        (&[("a", None), ("b", None), ("c", None)], None),
    );
    for stack in [forwards, backwards] {
        let cells = partials.allocated_cells();
        let result = stack
            .apply_partial_bindings(
                &mut partials,
                &PartialSymbolStackBindings::new(),
                &PartialScopeStackBindings::new(),
            )
            .unwrap();
        // The result is a new stack of three cells.  A reversal of the argument would add three
        // more.
        assert_eq!(cells + 3, partials.allocated_cells());
        assert!(result.equals(&mut partials, stack));
    }
}

/// Lists that share their first edges, as the paths of one stitcher run do.  Each pair holds a
/// common prefix, and two different tails.  The reference answer comes from the plain sequences.
#[test]
fn shadowing_both_ways_is_what_the_reference_says_in_every_direction() {
    let mut graph = StackGraph::new();
    let mut partials = PartialPaths::new();
    let sequences = all_edge_sequences();
    for left in sequences.iter().step_by(7) {
        for right in sequences.iter().step_by(5) {
            let expected = (expected_shadows(left, right), expected_shadows(right, left));
            for (left_forwards, right_forwards) in
                [(false, false), (true, false), (false, true), (true, true)]
            {
                let l = edge_list(&mut graph, &mut partials, left, left_forwards);
                let r = edge_list(&mut graph, &mut partials, right, right_forwards);
                assert_eq!(
                    expected,
                    l.shadows_both_ways(&mut partials, r),
                    "{:?} and {:?}, forwards: {} {}",
                    left,
                    right,
                    left_forwards,
                    right_forwards
                );
            }
        }
    }
}

/// Appends `tail` to a copy of `base`, at the back.  The result shares the cells of `base`.
fn extend_list(
    graph: &mut StackGraph,
    partials: &mut PartialPaths,
    base: PartialPathEdgeList,
    tail: &[(u32, i32)],
) -> PartialPathEdgeList {
    let file = graph.get_or_create_file("edges");
    let mut list = base;
    for (id, precedence) in tail {
        list.push_back(
            partials,
            stack_graphs::partial::PartialPathEdge {
                source_node_id: NodeID::new_in_file(file, *id),
                precedence: *precedence,
            },
        );
    }
    list
}

#[test]
fn shadowing_both_ways_of_lists_that_share_a_prefix_is_what_the_reference_says() {
    let mut graph = StackGraph::new();
    let mut partials = PartialPaths::new();
    let sequences = all_edge_sequences();
    // The prefixes include a precedence difference that decides, so that a wrong skip of the
    // shared cells changes the answer.
    for prefix in sequences.iter().step_by(11) {
        let base = edge_list(&mut graph, &mut partials, prefix, false);
        for left_tail in sequences.iter().step_by(13).take(30) {
            for right_tail in sequences.iter().step_by(17).take(30) {
                let l = extend_list(&mut graph, &mut partials, base, left_tail);
                let r = extend_list(&mut graph, &mut partials, base, right_tail);
                let left = [prefix.as_slice(), left_tail.as_slice()].concat();
                let right = [prefix.as_slice(), right_tail.as_slice()].concat();
                assert_eq!(
                    (
                        expected_shadows(&left, &right),
                        expected_shadows(&right, &left)
                    ),
                    l.shadows_both_ways(&mut partials, r),
                    "{:?} and {:?}",
                    left,
                    right
                );
                // The lists must also agree with the one-direction method.
                assert_eq!(
                    expected_shadows(&left, &right),
                    l.shadows(&mut partials, r),
                    "{:?} shadows {:?}",
                    left,
                    right
                );
            }
        }
    }
}

#[test]
fn shadowing_both_ways_can_hold_in_both_directions() {
    let mut graph = StackGraph::new();
    let mut partials = PartialPaths::new();
    // Each direction has its own deciding position: the first for the left list, and the second
    // for the right list.  The left list decides at the front, and the right list does not
    // decide before it.  So the right list decides at the second position only.
    let left = edge_list(&mut graph, &mut partials, &[(1, 1), (2, 0)], false);
    let right = edge_list(&mut graph, &mut partials, &[(1, 0), (2, 1)], false);
    assert_eq!((true, true), left.shadows_both_ways(&mut partials, right));
    // The position nearest to the front decides, so a later node change does not undo it.
    let left = edge_list(&mut graph, &mut partials, &[(1, 1), (2, 0)], false);
    let right = edge_list(&mut graph, &mut partials, &[(1, 0), (1, 0)], false);
    assert_eq!((true, false), left.shadows_both_ways(&mut partials, right));
    // An earlier node change decides "no" in both directions, over a later precedence.
    let left = edge_list(&mut graph, &mut partials, &[(1, 0), (2, 1)], false);
    let right = edge_list(&mut graph, &mut partials, &[(3, 0), (2, 0)], false);
    assert_eq!((false, false), left.shadows_both_ways(&mut partials, right));
}

#[test]
fn shadowing_both_ways_of_stitcher_lists_needs_no_new_cells() {
    let mut graph = StackGraph::new();
    let mut partials = PartialPaths::new();
    let base = edge_list(&mut graph, &mut partials, &[(1, 0), (2, 0)], false);
    let left = extend_list(&mut graph, &mut partials, base, &[(1, 1), (2, 2)]);
    let right = extend_list(&mut graph, &mut partials, base, &[(1, 0), (2, 0)]);
    let cells = partials.allocated_cells();
    assert_eq!((true, false), left.shadows_both_ways(&mut partials, right));
    assert_eq!(cells, partials.allocated_cells());
}

/// A graph for the tests of `can_be_shadowed`.  The path under test is
/// `reference (push x) -> scope -> definition (pop x)`.  The test adds rival edges.
struct ShadowGraph {
    graph: StackGraph,
    file: stack_graphs::arena::Handle<stack_graphs::graph::File>,
    reference: Handle<Node>,
    scope: Handle<Node>,
    definition: Handle<Node>,
}

impl ShadowGraph {
    fn new() -> ShadowGraph {
        let mut graph = StackGraph::new();
        let file = graph.add_file("shadow").unwrap();
        let reference = create_push_symbol_node(&mut graph, file, "x", true);
        let scope = create_scope_node(&mut graph, file, false);
        let definition = create_pop_symbol_node(&mut graph, file, "x", true);
        ShadowGraph {
            graph,
            file,
            reference,
            scope,
            definition,
        }
    }

    /// Makes the path with the precedence of its two edges.
    fn path(&mut self, partials: &mut PartialPaths, precedences: [i32; 2]) -> PartialPath {
        let nodes = [self.reference, self.scope, self.definition];
        let mut path = PartialPath::from_node(&self.graph, partials, nodes[0]);
        for (index, precedence) in precedences.iter().enumerate() {
            let edge = stack_graphs::graph::Edge {
                source: nodes[index],
                sink: nodes[index + 1],
                precedence: *precedence,
            };
            self.graph.add_edge(edge.source, edge.sink, edge.precedence);
            path.append(&self.graph, partials, edge).unwrap();
        }
        path
    }

    fn can_be_shadowed(&self, partials: &mut PartialPaths, path: &PartialPath) -> bool {
        path.can_be_shadowed(&self.graph, partials)
    }
}

#[test]
fn a_path_without_rival_edges_cannot_be_shadowed() {
    let mut g = ShadowGraph::new();
    let mut partials = PartialPaths::new();
    let path = g.path(&mut partials, [0, 0]);
    assert!(!g.can_be_shadowed(&mut partials, &path));
}

#[test]
fn an_empty_path_cannot_be_shadowed() {
    let mut g = ShadowGraph::new();
    let mut partials = PartialPaths::new();
    let path = PartialPath::from_node(&g.graph, &mut partials, g.reference);
    assert!(!g.can_be_shadowed(&mut partials, &path));
}

#[test]
fn a_rival_edge_of_a_higher_precedence_to_a_matching_pop_shadows() {
    let mut g = ShadowGraph::new();
    let mut partials = PartialPaths::new();
    let rival = create_pop_symbol_node(&mut g.graph, g.file, "x", true);
    g.graph.add_edge(g.scope, rival, 1);
    let path = g.path(&mut partials, [0, 0]);
    assert!(g.can_be_shadowed(&mut partials, &path));
}

#[test]
fn a_rival_edge_of_the_second_edge_position_is_found() {
    // The rival leaves the first node of the path, not the scope.
    let mut g = ShadowGraph::new();
    let mut partials = PartialPaths::new();
    let rival = create_pop_symbol_node(&mut g.graph, g.file, "x", true);
    g.graph.add_edge(g.reference, rival, 1);
    let path = g.path(&mut partials, [0, 0]);
    assert!(g.can_be_shadowed(&mut partials, &path));
}

#[test]
fn a_rival_edge_that_is_not_higher_cannot_shadow() {
    for rival_precedence in [-1, 0] {
        let mut g = ShadowGraph::new();
        let mut partials = PartialPaths::new();
        let rival = create_pop_symbol_node(&mut g.graph, g.file, "x", true);
        g.graph.add_edge(g.scope, rival, rival_precedence);
        let path = g.path(&mut partials, [0, 0]);
        assert!(
            !g.can_be_shadowed(&mut partials, &path),
            "rival precedence {}",
            rival_precedence
        );
    }
}

#[test]
fn a_rival_edge_must_be_higher_than_the_edge_that_the_path_takes() {
    // The path takes its edge at the scope with precedence 2.  A rival of precedence 2 does not
    // shadow it, and a rival of precedence 3 does.
    for (rival_precedence, expected) in [(2, false), (3, true)] {
        let mut g = ShadowGraph::new();
        let mut partials = PartialPaths::new();
        let rival = create_pop_symbol_node(&mut g.graph, g.file, "x", true);
        g.graph.add_edge(g.scope, rival, rival_precedence);
        let path = g.path(&mut partials, [0, 2]);
        assert_eq!(
            expected,
            g.can_be_shadowed(&mut partials, &path),
            "rival precedence {}",
            rival_precedence
        );
    }
}

#[test]
fn a_rival_edge_to_a_pop_of_another_symbol_cannot_shadow() {
    let mut g = ShadowGraph::new();
    let mut partials = PartialPaths::new();
    let rival = create_pop_symbol_node(&mut g.graph, g.file, "y", true);
    g.graph.add_edge(g.scope, rival, 1);
    let path = g.path(&mut partials, [0, 0]);
    assert!(!g.can_be_shadowed(&mut partials, &path));
}

#[test]
fn a_rival_edge_follows_scope_nodes_to_a_pop() {
    for (symbol, expected) in [("x", true), ("y", false)] {
        let mut g = ShadowGraph::new();
        let mut partials = PartialPaths::new();
        let middle = create_scope_node(&mut g.graph, g.file, false);
        let last = create_scope_node(&mut g.graph, g.file, false);
        let rival = create_pop_symbol_node(&mut g.graph, g.file, symbol, true);
        g.graph.add_edge(g.scope, middle, 1);
        g.graph.add_edge(middle, last, 0);
        g.graph.add_edge(last, rival, 0);
        let path = g.path(&mut partials, [0, 0]);
        assert_eq!(
            expected,
            g.can_be_shadowed(&mut partials, &path),
            "symbol {}",
            symbol
        );
    }
}

#[test]
fn a_rival_edge_to_a_node_that_is_not_a_scope_or_a_pop_may_shadow() {
    let mut g = ShadowGraph::new();
    let mut partials = PartialPaths::new();
    let rival = create_push_symbol_node(&mut g.graph, g.file, "z", false);
    g.graph.add_edge(g.scope, rival, 1);
    let path = g.path(&mut partials, [0, 0]);
    assert!(g.can_be_shadowed(&mut partials, &path));
}

#[test]
fn a_rival_search_that_runs_out_of_budget_may_shadow() {
    // A chain of 70 scope nodes with no pop: the search gives up and says "may shadow".
    // A short chain of the same shape that ends in a pop of another symbol says "cannot".
    for (length, expected) in [(70, true), (10, false)] {
        let mut g = ShadowGraph::new();
        let mut partials = PartialPaths::new();
        let mut previous = g.scope;
        let mut precedence = 1;
        for _ in 0..length {
            let next = create_scope_node(&mut g.graph, g.file, false);
            g.graph.add_edge(previous, next, precedence);
            previous = next;
            precedence = 0;
        }
        if length < 64 {
            let rival = create_pop_symbol_node(&mut g.graph, g.file, "y", true);
            g.graph.add_edge(previous, rival, 0);
        }
        let path = g.path(&mut partials, [0, 0]);
        assert_eq!(
            expected,
            g.can_be_shadowed(&mut partials, &path),
            "chain of {}",
            length
        );
    }
}

#[test]
fn the_symbol_stack_follows_the_edges_of_the_path() {
    // The path starts at the scope, and the definition pops x.  So the precondition holds x, and
    // the stack at the scope has x on top.
    let mut g = ShadowGraph::new();
    let mut partials = PartialPaths::new();
    let rival = create_pop_symbol_node(&mut g.graph, g.file, "y", true);
    g.graph.add_edge(g.scope, rival, 1);
    let mut path = PartialPath::from_node(&g.graph, &mut partials, g.scope);
    let edge = stack_graphs::graph::Edge {
        source: g.scope,
        sink: g.definition,
        precedence: 0,
    };
    g.graph.add_edge(edge.source, edge.sink, edge.precedence);
    path.append(&g.graph, &mut partials, edge).unwrap();
    // The rival pops y, and the top is x.
    assert!(!g.can_be_shadowed(&mut partials, &path));

    // A rival that pops x matches the x that the path needs.
    let rival_x = create_pop_symbol_node(&mut g.graph, g.file, "x", true);
    g.graph.add_edge(g.scope, rival_x, 1);
    assert!(g.can_be_shadowed(&mut partials, &path));
}

#[test]
fn the_top_of_the_stack_at_a_rival_follows_the_pushes_and_pops_of_the_path() {
    // The path is: push y, push x, scope a, pop x, scope b, pop y.  The top of the stack is x at
    // `a` and y at `b`.  The rival at `a` or `b` shadows only when it pops that symbol.
    for (at_a, symbol, expected) in [
        (true, "x", true),
        (true, "y", false),
        (false, "y", true),
        (false, "x", false),
    ] {
        let mut g = ShadowGraph::new();
        let mut partials = PartialPaths::new();
        let push_y = create_push_symbol_node(&mut g.graph, g.file, "y", true);
        let push_x = create_push_symbol_node(&mut g.graph, g.file, "x", false);
        let a = create_scope_node(&mut g.graph, g.file, false);
        let pop_x = create_pop_symbol_node(&mut g.graph, g.file, "x", false);
        let b = create_scope_node(&mut g.graph, g.file, false);
        let pop_y = create_pop_symbol_node(&mut g.graph, g.file, "y", true);
        let nodes = [push_y, push_x, a, pop_x, b, pop_y];
        let rival = create_pop_symbol_node(&mut g.graph, g.file, symbol, true);
        g.graph.add_edge(if at_a { a } else { b }, rival, 1);
        let mut path = PartialPath::from_node(&g.graph, &mut partials, nodes[0]);
        for pair in nodes.windows(2) {
            let edge = stack_graphs::graph::Edge {
                source: pair[0],
                sink: pair[1],
                precedence: 0,
            };
            g.graph.add_edge(edge.source, edge.sink, edge.precedence);
            path.append(&g.graph, &mut partials, edge).unwrap();
        }
        assert_eq!(
            expected,
            g.can_be_shadowed(&mut partials, &path),
            "rival at {} pops {}",
            if at_a { "a" } else { "b" },
            symbol
        );
    }
}

#[test]
fn a_path_with_an_unknown_stack_and_no_symbol_on_top_can_be_shadowed_by_any_pop() {
    let mut g = ShadowGraph::new();
    let mut partials = PartialPaths::new();
    let a = create_scope_node(&mut g.graph, g.file, false);
    let b = create_scope_node(&mut g.graph, g.file, false);
    let rival = create_pop_symbol_node(&mut g.graph, g.file, "y", true);
    g.graph.add_edge(a, rival, 1);
    let mut path = PartialPath::from_node(&g.graph, &mut partials, a);
    let edge = stack_graphs::graph::Edge {
        source: a,
        sink: b,
        precedence: 0,
    };
    g.graph.add_edge(a, b, 0);
    path.append(&g.graph, &mut partials, edge).unwrap();
    // The precondition has a variable: the stack below is unknown, so the pop can match.
    assert!(g.can_be_shadowed(&mut partials, &path));
    // With no variable, the stack is empty, and no pop can match.
    path.eliminate_precondition_stack_variables(&mut partials);
    assert!(!g.can_be_shadowed(&mut partials, &path));
}

#[test]
fn a_path_over_an_edge_from_a_missing_node_may_be_shadowed() {
    let mut g = ShadowGraph::new();
    let mut partials = PartialPaths::new();
    let mut path = g.path(&mut partials, [0, 0]);
    let file = g.graph.get_or_create_file("elsewhere");
    let missing = stack_graphs::partial::PartialPathEdge {
        source_node_id: NodeID::new_in_file(file, 999),
        precedence: 0,
    };
    // Last edge: the node of the edge is missing.
    path.edges.push_back(&mut partials, missing);
    assert!(g.can_be_shadowed(&mut partials, &path));
    // First edge: the node of the first edge is missing.
    let mut path = PartialPath::from_node(&g.graph, &mut partials, g.reference);
    path.edges.push_back(&mut partials, missing);
    assert!(g.can_be_shadowed(&mut partials, &path));
}

#[test]
fn can_be_shadowed_reads_a_list_in_both_directions() {
    // A path whose edge list is stored forwards gives the same answer as one stored backwards.
    let mut g = ShadowGraph::new();
    let mut partials = PartialPaths::new();
    let rival = create_pop_symbol_node(&mut g.graph, g.file, "x", true);
    g.graph.add_edge(g.scope, rival, 1);
    let mut path = g.path(&mut partials, [0, 0]);
    let backwards = g.can_be_shadowed(&mut partials, &path);
    let first = path.edges.pop_front(&mut partials).unwrap();
    path.edges.push_front(&mut partials, first);
    let forwards = g.can_be_shadowed(&mut partials, &path);
    assert!(backwards);
    assert_eq!(backwards, forwards);
}

#[test]
fn the_symbol_stack_follows_the_pushes_and_pops_along_the_path() {
    // The path is `push x -> pop x -> m -> push y -> a -> e`, with known stacks.  The stack at
    // `m` is empty, and the stack at `a` has y on top.  A rival edge from a node can shadow only
    // if it can reach a pop of the symbol that is on top at that node.
    for (rival_at, symbol, expected) in [
        ("m", "x", false),
        ("m", "y", false),
        ("a", "y", true),
        ("a", "x", false),
    ] {
        let mut graph = StackGraph::new();
        let mut partials = PartialPaths::new();
        let file = graph.add_file("stack").unwrap();
        let push_x = create_push_symbol_node(&mut graph, file, "x", true);
        let pop_x = create_pop_symbol_node(&mut graph, file, "x", false);
        let m = create_scope_node(&mut graph, file, false);
        let push_y = create_push_symbol_node(&mut graph, file, "y", false);
        let a = create_scope_node(&mut graph, file, false);
        let e = create_scope_node(&mut graph, file, false);
        let nodes = [push_x, pop_x, m, push_y, a, e];
        let mut path = PartialPath::from_node(&graph, &mut partials, nodes[0]);
        for pair in nodes.windows(2) {
            let edge = stack_graphs::graph::Edge {
                source: pair[0],
                sink: pair[1],
                precedence: 0,
            };
            graph.add_edge(edge.source, edge.sink, 0);
            path.append(&graph, &mut partials, edge).unwrap();
        }
        path.eliminate_precondition_stack_variables(&mut partials);
        let rival = create_pop_symbol_node(&mut graph, file, symbol, true);
        graph.add_edge(if rival_at == "m" { m } else { a }, rival, 1);
        assert_eq!(
            expected,
            path.can_be_shadowed(&graph, &mut partials),
            "rival at {} pops {}",
            rival_at,
            symbol
        );
    }
}
