use std::collections::HashMap;

use crate::ast::Selection;
use crate::error::Result;
use crate::interpreter::Transform;
use crate::value::{Array, Level, Value};

use super::group::extract_key;

fn dedupe_with_counts_by<F, G>(arr: Array, key_fn: F, output_fn: G) -> Result<Value>
where
    F: Fn(&Value) -> Result<String>,
    G: Fn(&Value) -> Result<Value>,
{
    let cap = arr.elements.len() / 2;
    let mut index_map: HashMap<String, usize> = HashMap::with_capacity(cap);
    let mut entries: Vec<(usize, Value)> = Vec::with_capacity(cap);

    for elem in arr.elements {
        let key = key_fn(&elem)?;
        if let Some(&idx) = index_map.get(&key) {
            entries[idx].0 += 1;
        } else {
            let idx = entries.len();
            index_map.insert(key, idx);
            entries.push((1, elem));
        }
    }

    let mut result: Vec<(usize, usize, Value)> = entries
        .into_iter()
        .enumerate()
        .map(|(order, (count, v))| {
            let output = output_fn(&v).unwrap();
            (count, order, output)
        })
        .collect();

    result.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));

    let elements: Vec<Value> = result
        .into_iter()
        .map(|(count, _, v)| {
            Value::Array(Array::from((
                vec![Value::Number(count as f64), v],
                Level::Word,
            )))
        })
        .collect();

    Ok(Value::Array(Array::from((elements, Level::Line))))
}

pub struct DedupeWithCounts;

impl Transform for DedupeWithCounts {
    fn apply(&self, value: Value) -> Result<Value> {
        match value {
            Value::Array(arr) => dedupe_with_counts_by(
                arr,
                |elem| Ok(value_to_key(elem)),
                |elem| Ok(elem.deep_copy()),
            ),
            other => Ok(other),
        }
    }

    fn requires_full_input(&self) -> bool {
        true
    }
}

pub fn value_to_key(value: &Value) -> String {
    match value {
        Value::Text(s) => format!("T:{}", s),
        Value::Number(n) => format!("N:{}", n),
        Value::Array(arr) => {
            let inner: Vec<String> = arr.elements.iter().map(value_to_key).collect();
            format!("A:[{}]", inner.join(","))
        }
    }
}

pub struct DedupeSelectionWithCounts {
    selection: Selection,
}

impl DedupeSelectionWithCounts {
    pub fn new(selection: Selection) -> Self {
        Self { selection }
    }
}

impl Transform for DedupeSelectionWithCounts {
    fn apply(&self, value: Value) -> Result<Value> {
        match value {
            Value::Array(arr) => dedupe_with_counts_by(
                arr,
                |elem| {
                    let extracted = extract_key(elem, &self.selection)?;
                    Ok(value_to_key(&extracted))
                },
                |elem| extract_key(elem, &self.selection),
            ),
            other => Ok(other),
        }
    }

    fn requires_full_input(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::SelectItem;

    fn text(s: &str) -> Value {
        Value::Text(s.to_string())
    }

    #[test]
    fn dedupe_with_counts_basic() {
        let input = Value::Array(Array::from((
            vec![text("a"), text("b"), text("a"), text("a"), text("b")],
            Level::Line,
        )));
        let result = DedupeWithCounts.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 2);
                match &arr.elements[0] {
                    Value::Array(inner) => {
                        assert_eq!(inner.len(), 2);
                        assert_eq!(inner.elements[0], Value::Number(3.0));
                        assert_eq!(inner.elements[1], text("a"));
                    }
                    _ => panic!("expected inner array"),
                }
                match &arr.elements[1] {
                    Value::Array(inner) => {
                        assert_eq!(inner.len(), 2);
                        assert_eq!(inner.elements[0], Value::Number(2.0));
                        assert_eq!(inner.elements[1], text("b"));
                    }
                    _ => panic!("expected inner array"),
                }
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn dedupe_with_counts_preserves_order_for_ties() {
        let input = Value::Array(Array::from((
            vec![text("x"), text("y"), text("z")],
            Level::Line,
        )));
        let result = DedupeWithCounts.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 3);
                match &arr.elements[0] {
                    Value::Array(inner) => {
                        assert_eq!(inner.elements[1], text("x"));
                    }
                    _ => panic!("expected inner array"),
                }
                match &arr.elements[1] {
                    Value::Array(inner) => {
                        assert_eq!(inner.elements[1], text("y"));
                    }
                    _ => panic!("expected inner array"),
                }
                match &arr.elements[2] {
                    Value::Array(inner) => {
                        assert_eq!(inner.elements[1], text("z"));
                    }
                    _ => panic!("expected inner array"),
                }
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn dedupe_with_counts_numbers() {
        let input = Value::Array(Array::from((
            vec![Value::Number(1.0), Value::Number(2.0), Value::Number(1.0)],
            Level::Line,
        )));
        let result = DedupeWithCounts.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 2);
                match &arr.elements[0] {
                    Value::Array(inner) => {
                        assert_eq!(inner.elements[0], Value::Number(2.0));
                        assert_eq!(inner.elements[1], Value::Number(1.0));
                    }
                    _ => panic!("expected inner array"),
                }
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn dedupe_with_counts_empty_array() {
        let input = Value::Array(Array::from((vec![], Level::Line)));
        let result = DedupeWithCounts.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert!(arr.is_empty());
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn dedupe_with_counts_non_array_is_identity() {
        let input = text("hello");
        let result = DedupeWithCounts.apply(input).unwrap();
        assert_eq!(result, text("hello"));
    }

    #[test]
    fn dedupe_selection_with_counts_basic() {
        // Array of [key, value] pairs, dedupe by key (index 0)
        let input = Value::Array(Array::from((
            vec![
                Value::Array(Array::from((
                    vec![text("a"), Value::Number(1.0)],
                    Level::Word,
                ))),
                Value::Array(Array::from((
                    vec![text("b"), Value::Number(2.0)],
                    Level::Word,
                ))),
                Value::Array(Array::from((
                    vec![text("a"), Value::Number(3.0)],
                    Level::Word,
                ))),
            ],
            Level::Line,
        )));
        let dedupe = DedupeSelectionWithCounts::new(Selection {
            items: vec![SelectItem::Index(0)],
        });
        let result = dedupe.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 2);
                // First result: count=2, key="a"
                match &arr.elements[0] {
                    Value::Array(inner) => {
                        assert_eq!(inner.elements[0], Value::Number(2.0));
                        assert_eq!(inner.elements[1], text("a"));
                    }
                    _ => panic!("expected inner array"),
                }
                // Second result: count=1, key="b"
                match &arr.elements[1] {
                    Value::Array(inner) => {
                        assert_eq!(inner.elements[0], Value::Number(1.0));
                        assert_eq!(inner.elements[1], text("b"));
                    }
                    _ => panic!("expected inner array"),
                }
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn dedupe_selection_with_counts_preserves_order_for_ties() {
        let input = Value::Array(Array::from((
            vec![
                Value::Array(Array::from((vec![text("x"), text("1")], Level::Word))),
                Value::Array(Array::from((vec![text("y"), text("2")], Level::Word))),
                Value::Array(Array::from((vec![text("z"), text("3")], Level::Word))),
            ],
            Level::Line,
        )));
        let dedupe = DedupeSelectionWithCounts::new(Selection {
            items: vec![SelectItem::Index(0)],
        });
        let result = dedupe.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 3);
                // All have count 1, so order should be preserved
                match &arr.elements[0] {
                    Value::Array(inner) => {
                        assert_eq!(inner.elements[1], text("x"));
                    }
                    _ => panic!("expected array"),
                }
                match &arr.elements[1] {
                    Value::Array(inner) => {
                        assert_eq!(inner.elements[1], text("y"));
                    }
                    _ => panic!("expected array"),
                }
                match &arr.elements[2] {
                    Value::Array(inner) => {
                        assert_eq!(inner.elements[1], text("z"));
                    }
                    _ => panic!("expected array"),
                }
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn dedupe_selection_with_counts_empty_array() {
        let input = Value::Array(Array::from((vec![], Level::Line)));
        let dedupe = DedupeSelectionWithCounts::new(Selection {
            items: vec![SelectItem::Index(0)],
        });
        let result = dedupe.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert!(arr.is_empty());
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn dedupe_selection_with_counts_non_array_is_identity() {
        let input = text("hello");
        let dedupe = DedupeSelectionWithCounts::new(Selection {
            items: vec![SelectItem::Index(0)],
        });
        let result = dedupe.apply(input).unwrap();
        assert_eq!(result, text("hello"));
    }
}
