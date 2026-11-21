use tree_sitter_stack_graphs_python::incremental::{
    lower_incremental_document, Completeness, Expression, SemanticKind,
};

#[test]
fn ternary_summary_excludes_condition() {
    let output = lower_incremental_document(
        "m",
        "m.py",
        "def choose(x, y, condition):\n    return x if condition else y\n",
    );
    let ret = output
        .fragment
        .items
        .iter()
        .find(|item| matches!(item.kind, SemanticKind::Return))
        .unwrap();
    assert_eq!(
        ret.expression,
        Some(Expression::Conditional {
            branches: vec![Expression::Name("x".into()), Expression::Name("y".into())]
        })
    );
}

#[test]
fn unsupported_expression_marks_document_incomplete_and_keeps_valid_facts() {
    let output = lower_incremental_document(
        "m",
        "m.py",
        "def value(x):\n    return x + 1\n\ndef later():\n    return 2\n",
    );
    assert_eq!(output.completeness, Completeness::Incomplete);
    assert!(output
        .diagnostics
        .iter()
        .any(|d| d.message.contains("binary_operator")));
    assert!(output
        .fragment
        .items
        .iter()
        .any(|i| i.name.as_deref() == Some("later") && matches!(i.kind, SemanticKind::Function)));
}

#[test]
fn nested_calls_and_names_have_current_ranges() {
    let output = lower_incremental_document("m", "m.py", "def f(x):\n    return outer(inner(x))\n");
    let calls = output
        .fragment
        .items
        .iter()
        .filter(|i| matches!(i.kind, SemanticKind::Call))
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    for call in calls {
        assert!(output
            .source_map
            .entries
            .iter()
            .any(|range| range.item_id == call.id));
    }
    assert!(output
        .fragment
        .items
        .iter()
        .any(|i| i.name.as_deref() == Some("x") && matches!(i.kind, SemanticKind::Name)));
}

#[test]
fn parser_recovery_keeps_later_valid_declarations() {
    let output = lower_incremental_document(
        "m",
        "m.py",
        "def broken(:\n    pass\n\ndef valid():\n    return 1\n",
    );
    assert_eq!(output.completeness, Completeness::Incomplete);
    assert!(!output.diagnostics.is_empty());
    assert!(output
        .fragment
        .items
        .iter()
        .any(|i| i.name.as_deref() == Some("valid") && matches!(i.kind, SemanticKind::Function)));
}

#[test]
fn malformed_function_without_error_node_is_incomplete() {
    let output = lower_incremental_document("m", "m.py", "def pick(:\n    return 42\n");
    assert_eq!(output.completeness, Completeness::Incomplete);
    assert!(!output.diagnostics.is_empty());
}

#[test]
fn if_conditions_do_not_become_return_values_and_arms_keep_identity() {
    let output = lower_incremental_document(
        "m",
        "m.py",
        "def choose(flag, left, right):\n    if flag == 0:\n        return left\n    else:\n        return right\n",
    );
    let returns = output
        .fragment
        .items
        .iter()
        .filter(|item| matches!(item.kind, SemanticKind::Return))
        .collect::<Vec<_>>();
    assert_eq!(returns.len(), 2);
    assert_ne!(returns[0].parent_id, returns[1].parent_id);
    assert!(returns
        .iter()
        .all(|item| item.expression.as_ref() != Some(&Expression::Name("flag".into()))));
}
