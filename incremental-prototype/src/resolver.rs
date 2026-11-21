//! Bounded symbolic summary composition and least fixed-point solver.
//!
//! The solver starts from empty results for every call. It grows result sets by
//! union until the sets stop changing. A fresh call to `solve` rebuilds the
//! recursive group from its current formulas, so removed support cannot remain.
//! Symbolic terms use `$arg[n]` and `$receiver` markers. A finite string and set
//! bound, plus the iteration limit, turns excess work into an incomplete result.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

const MAX_SYMBOLIC_BYTES: usize = 512;
const MAX_VALUES_PER_SUMMARY: usize = 1024;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum Formula {
    Argument(usize),
    Receiver,
    Literal(String),
    Call(String, Vec<Formula>),
    Choice(Vec<Formula>),
    Unknown,
    Member(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum Value {
    Literal(String),
    Symbolic(String),
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FunctionSummary {
    pub params: usize,
    pub body: Formula,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SolvedSummary {
    pub values: BTreeSet<Value>,
    pub complete: bool,
    pub iterations: usize,
}

#[derive(Default)]
struct Evaluation {
    values: BTreeSet<Value>,
    complete: bool,
    exceeded_bound: bool,
}

/// Evaluate a formula with explicit arguments, receiver state, and solved calls.
pub fn apply(
    formula: &Formula,
    args: &[Value],
    receiver: Option<&Value>,
    summaries: &BTreeMap<String, SolvedSummary>,
) -> BTreeSet<Value> {
    evaluate(formula, args, receiver, summaries, None).values
}

/// Compute the least fixed point for the supplied function group.
///
/// Every call starts with empty result sets. `limit` bounds fixed-point rounds.
/// A missing call, explicit Unknown, excess symbolic growth, or exhausted round
/// limit marks the affected summaries incomplete.
pub fn solve(
    functions: &BTreeMap<String, FunctionSummary>,
    limit: usize,
) -> BTreeMap<String, SolvedSummary> {
    let mut current: BTreeMap<String, SolvedSummary> = functions
        .keys()
        .map(|name| {
            (
                name.clone(),
                SolvedSummary {
                    values: BTreeSet::new(),
                    complete: true,
                    iterations: 0,
                },
            )
        })
        .collect();

    for iteration in 1..=limit {
        let mut next = current.clone();
        for (name, function) in functions {
            let args = symbolic_args(function.params);
            let receiver = Value::Symbolic("$receiver".into());
            let result = evaluate(
                &function.body,
                &args,
                Some(&receiver),
                &current,
                Some(functions),
            );
            let summary = next
                .get_mut(name)
                .expect("all functions have an initial summary");
            summary.values.extend(result.values);
            summary.complete &= result.complete && !result.exceeded_bound;
            summary.iterations = iteration;
            if summary.values.len() > MAX_VALUES_PER_SUMMARY {
                summary.values.clear();
                summary.values.insert(Value::Unknown);
                summary.complete = false;
            }
        }

        let stable = next.iter().all(|(name, summary)| {
            current
                .get(name)
                .is_some_and(|old| old.values == summary.values && old.complete == summary.complete)
        });
        current = next;
        if stable {
            return current;
        }
    }

    // We cannot publish a partial recursive approximation as complete.
    for summary in current.values_mut() {
        summary.complete = false;
    }
    current
}

fn symbolic_args(count: usize) -> Vec<Value> {
    (0..count)
        .map(|index| Value::Symbolic(format!("$arg[{index}]")))
        .collect()
}

fn uses_receiver(formula: &Formula) -> bool {
    match formula {
        Formula::Receiver | Formula::Member(_) => true,
        Formula::Call(_, args) | Formula::Choice(args) => args.iter().any(uses_receiver),
        Formula::Argument(_) | Formula::Literal(_) | Formula::Unknown => false,
    }
}

fn uses_argument_at_or_after(formula: &Formula, index: usize) -> bool {
    match formula {
        Formula::Argument(argument) => *argument >= index,
        Formula::Call(_, args) | Formula::Choice(args) => args
            .iter()
            .any(|item| uses_argument_at_or_after(item, index)),
        Formula::Receiver | Formula::Literal(_) | Formula::Unknown | Formula::Member(_) => false,
    }
}

fn evaluate(
    formula: &Formula,
    args: &[Value],
    receiver: Option<&Value>,
    summaries: &BTreeMap<String, SolvedSummary>,
    functions: Option<&BTreeMap<String, FunctionSummary>>,
) -> Evaluation {
    match formula {
        Formula::Argument(index) => match args.get(*index) {
            Some(value) => Evaluation {
                values: BTreeSet::from([value.clone()]),
                complete: !matches!(value, Value::Unknown),
                exceeded_bound: false,
            },
            None => unknown_incomplete(),
        },
        Formula::Receiver => match receiver {
            Some(value) => Evaluation {
                values: BTreeSet::from([value.clone()]),
                complete: !matches!(value, Value::Unknown),
                exceeded_bound: false,
            },
            None => unknown_incomplete(),
        },
        Formula::Literal(value) => Evaluation {
            values: BTreeSet::from([Value::Literal(value.clone())]),
            complete: true,
            exceeded_bound: false,
        },
        Formula::Member(member) => match receiver {
            Some(Value::Literal(value)) | Some(Value::Symbolic(value)) => {
                let result = format!("{value}.{member}");
                bounded_symbolic(result)
            }
            Some(Value::Unknown) | None => unknown_incomplete(),
        },
        Formula::Choice(items) => {
            let mut result = Evaluation {
                values: BTreeSet::new(),
                complete: true,
                exceeded_bound: false,
            };
            for item in items {
                let branch = evaluate(item, args, receiver, summaries, functions);
                result.values.extend(branch.values);
                result.complete &= branch.complete;
                result.exceeded_bound |= branch.exceeded_bound;
            }
            bound_values(result)
        }
        Formula::Call(name, actuals) => {
            let Some(callee) = summaries.get(name) else {
                return unknown_incomplete();
            };
            let mut evaluated_actuals = Vec::with_capacity(actuals.len() + 1);
            let mut complete = callee.complete;
            let mut exceeded_bound = false;
            let implicit_receiver = functions
                .and_then(|items| items.get(name))
                .map(|function| {
                    actuals.len() + 1 == function.params
                        && (uses_receiver(&function.body)
                            || uses_argument_at_or_after(&function.body, actuals.len()))
                })
                .unwrap_or_else(|| {
                    receiver.is_some()
                        && callee.values.iter().any(|value| match value {
                            Value::Symbolic(symbol) => (actuals.len()..MAX_VALUES_PER_SUMMARY)
                                .any(|index| symbol.contains(&format!("$arg[{index}]"))),
                            _ => false,
                        })
                });
            if implicit_receiver {
                if let Some(receiver) = receiver {
                    evaluated_actuals.push(BTreeSet::from([receiver.clone()]));
                }
            }
            for actual in actuals {
                let value = evaluate(actual, args, receiver, summaries, functions);
                complete &= value.complete;
                exceeded_bound |= value.exceeded_bound;
                evaluated_actuals.push(value.values);
            }
            if functions
                .and_then(|items| items.get(name))
                .is_some_and(|function| evaluated_actuals.len() != function.params)
            {
                complete = false;
            }
            let mut values = BTreeSet::new();
            for value in &callee.values {
                let substitutions = substitute(value, &evaluated_actuals, receiver);
                values.extend(substitutions.values);
                complete &= substitutions.complete;
                exceeded_bound |= substitutions.exceeded_bound;
            }
            bound_values(Evaluation {
                values,
                complete,
                exceeded_bound,
            })
        }
        Formula::Unknown => unknown_incomplete(),
    }
}

fn substitute(value: &Value, actuals: &[BTreeSet<Value>], receiver: Option<&Value>) -> Evaluation {
    let Value::Symbolic(symbol) = value else {
        return Evaluation {
            values: BTreeSet::from([value.clone()]),
            complete: !matches!(value, Value::Unknown),
            exceeded_bound: false,
        };
    };

    let receiver_values = receiver.cloned().map(|value| BTreeSet::from([value]));
    let Some((start, end, actual_values)) = find_marker(symbol, actuals, receiver_values.as_ref())
    else {
        if has_unbound_marker(symbol, actuals.len(), receiver.is_some()) {
            return unknown_incomplete();
        }
        return bounded_symbolic(symbol.clone());
    };
    let prefix = &symbol[..start];
    let suffix = &symbol[end..];
    let mut result = Evaluation {
        values: BTreeSet::new(),
        complete: true,
        exceeded_bound: false,
    };
    for actual in actual_values {
        match actual {
            Value::Unknown => {
                result.values.insert(Value::Unknown);
                result.complete = false;
            }
            Value::Literal(actual) if prefix.is_empty() && suffix.is_empty() => {
                result.values.insert(Value::Literal(actual.clone()));
            }
            Value::Literal(actual) | Value::Symbolic(actual) => {
                let combined = format!("{prefix}{actual}{suffix}");
                if combined == *symbol {
                    result.values.insert(Value::Symbolic(combined));
                    continue;
                }
                let nested = substitute(&Value::Symbolic(combined), actuals, receiver);
                result.values.extend(nested.values);
                result.complete &= nested.complete;
                result.exceeded_bound |= nested.exceeded_bound;
            }
        }
    }
    bound_values(result)
}

fn find_marker<'a>(
    symbol: &str,
    actuals: &'a [BTreeSet<Value>],
    receiver_values: Option<&'a BTreeSet<Value>>,
) -> Option<(usize, usize, &'a BTreeSet<Value>)> {
    let mut found: Option<(usize, usize, &BTreeSet<Value>)> = None;
    for (index, values) in actuals.iter().enumerate() {
        let marker = format!("$arg[{index}]");
        if let Some(start) = symbol.find(&marker) {
            if found.as_ref().is_none_or(|old| start < old.0) {
                found = Some((start, start + marker.len(), values));
            }
        }
    }
    if let (Some(start), Some(values)) = (symbol.find("$receiver"), receiver_values) {
        if found.as_ref().is_none_or(|old| start < old.0) {
            found = Some((start, start + "$receiver".len(), values));
        }
    }
    found
}

fn has_unbound_marker(symbol: &str, argument_count: usize, has_receiver: bool) -> bool {
    if !has_receiver && symbol.contains("$receiver") {
        return true;
    }
    (argument_count..MAX_VALUES_PER_SUMMARY).any(|index| symbol.contains(&format!("$arg[{index}]")))
}

fn bounded_symbolic(value: String) -> Evaluation {
    if value.len() > MAX_SYMBOLIC_BYTES {
        unknown_incomplete_with_bound()
    } else {
        Evaluation {
            values: BTreeSet::from([Value::Symbolic(value)]),
            complete: true,
            exceeded_bound: false,
        }
    }
}

fn bound_values(mut result: Evaluation) -> Evaluation {
    if result.values.len() > MAX_VALUES_PER_SUMMARY {
        result.values.clear();
        result.values.insert(Value::Unknown);
        result.complete = false;
        result.exceeded_bound = true;
    }
    result
}

fn unknown_incomplete() -> Evaluation {
    Evaluation {
        values: BTreeSet::from([Value::Unknown]),
        complete: false,
        exceeded_bound: false,
    }
}

fn unknown_incomplete_with_bound() -> Evaluation {
    Evaluation {
        values: BTreeSet::from([Value::Unknown]),
        complete: false,
        exceeded_bound: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn functions(
        items: impl IntoIterator<Item = (String, FunctionSummary)>,
    ) -> BTreeMap<String, FunctionSummary> {
        items.into_iter().collect()
    }

    fn values(items: impl IntoIterator<Item = Value>) -> BTreeSet<Value> {
        items.into_iter().collect()
    }

    #[test]
    fn identity_preserves_two_distinct_call_contexts() {
        let functions = functions([(
            "identity".into(),
            FunctionSummary {
                params: 1,
                body: Formula::Argument(0),
            },
        )]);
        let solved = solve(&functions, 8);
        let left = apply(
            &Formula::Call("identity".into(), vec![Formula::Literal("left".into())]),
            &[],
            None,
            &solved,
        );
        let right = apply(
            &Formula::Call("identity".into(), vec![Formula::Literal("right".into())]),
            &[],
            None,
            &solved,
        );
        assert_eq!(left, values([Value::Literal("left".into())]));
        assert_eq!(right, values([Value::Literal("right".into())]));
        assert!(solved["identity"].complete);
    }

    #[test]
    fn nested_calls_compose_actual_arguments() {
        let functions = functions([
            (
                "identity".into(),
                FunctionSummary {
                    params: 1,
                    body: Formula::Argument(0),
                },
            ),
            (
                "wrapper".into(),
                FunctionSummary {
                    params: 1,
                    body: Formula::Call("identity".into(), vec![Formula::Argument(0)]),
                },
            ),
        ]);
        let solved = solve(&functions, 8);
        let output = apply(
            &Formula::Call("wrapper".into(), vec![Formula::Literal("payload".into())]),
            &[],
            None,
            &solved,
        );
        assert_eq!(output, values([Value::Literal("payload".into())]));
        assert!(solved.values().all(|summary| summary.complete));
    }

    #[test]
    fn recursive_seed_support_is_removed_by_fresh_recomputation() {
        let seeded = functions([(
            "walk".into(),
            FunctionSummary {
                params: 1,
                body: Formula::Choice(vec![
                    Formula::Argument(0),
                    Formula::Call("walk".into(), vec![Formula::Argument(0)]),
                ]),
            },
        )]);
        let supported = solve(&seeded, 8);
        assert_eq!(
            supported["walk"].values,
            values([Value::Symbolic("$arg[0]".into())])
        );

        let unseeded = functions([(
            "walk".into(),
            FunctionSummary {
                params: 1,
                body: Formula::Call("walk".into(), vec![Formula::Argument(0)]),
            },
        )]);
        let deleted = solve(&unseeded, 8);
        assert!(deleted["walk"].values.is_empty());
        assert!(deleted["walk"].complete);
    }

    #[test]
    fn receiver_member_summary_uses_each_receiver() {
        let functions = functions([(
            "get".into(),
            FunctionSummary {
                params: 1,
                body: Formula::Member("value".into()),
            },
        )]);
        let solved = solve(&functions, 8);
        let left = apply(
            &Formula::Call("get".into(), vec![]),
            &[],
            Some(&Value::Symbolic("left".into())),
            &solved,
        );
        let right = apply(
            &Formula::Call("get".into(), vec![]),
            &[],
            Some(&Value::Symbolic("right".into())),
            &solved,
        );
        assert_eq!(left, values([Value::Symbolic("left.value".into())]));
        assert_eq!(right, values([Value::Symbolic("right.value".into())]));
    }

    #[test]
    fn iteration_limit_marks_recursive_group_incomplete() {
        let functions = functions([
            (
                "a".into(),
                FunctionSummary {
                    params: 0,
                    body: Formula::Call("b".into(), vec![]),
                },
            ),
            (
                "b".into(),
                FunctionSummary {
                    params: 0,
                    body: Formula::Literal("seed".into()),
                },
            ),
        ]);
        let solved = solve(&functions, 1);
        assert!(solved.values().all(|summary| !summary.complete));
    }

    #[test]
    fn explicit_unknown_is_incomplete() {
        let functions = functions([(
            "unknown".into(),
            FunctionSummary {
                params: 0,
                body: Formula::Unknown,
            },
        )]);
        let solved = solve(&functions, 8);
        assert!(!solved["unknown"].complete);
        assert_eq!(solved["unknown"].values, values([Value::Unknown]));
    }
}
