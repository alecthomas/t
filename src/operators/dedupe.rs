use std::collections::HashMap;

use crate::error::Result;
use crate::interpreter::Transform;
use crate::value::{Array, Level, Value};

pub struct DedupeWithCounts;

impl Transform for DedupeWithCounts {
    fn apply(&self, value: Value) -> Result<Value> {
        match value {
            Value::Array(arr) => {
                let mut counts: HashMap<String, (usize, usize)> = HashMap::new();
                let mut values: Vec<Value> = Vec::new();

                for elem in arr.elements {
                    let key = value_to_key(&elem);
                    if let Some((count, _)) = counts.get_mut(&key) {
                        *count += 1;
                    } else {
                        let order = values.len();
                        counts.insert(key, (1, order));
                        values.push(elem);
                    }
                }

                let mut result: Vec<(usize, usize, Value)> = values
                    .into_iter()
                    .map(|v| {
                        let key = value_to_key(&v);
                        let (count, order) = counts[&key];
                        (count, order, v)
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

pub struct Dedupe;

impl Transform for Dedupe {
    fn apply(&self, value: Value) -> Result<Value> {
        match value {
            Value::Array(arr) => {
                let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
                let mut result: Vec<Value> = Vec::new();

                for elem in arr.elements {
                    let key = value_to_key(&elem);
                    if seen.insert(key) {
                        result.push(elem);
                    }
                }

                Ok(Value::Array(Array::from((result, arr.level))))
            }
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
    fn dedupe_basic() {
        let input = Value::Array(Array::from((
            vec![text("a"), text("b"), text("a"), text("a"), text("b")],
            Level::Line,
        )));
        let result = Dedupe.apply(input).unwrap();
        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 2);
                assert_eq!(arr.elements[0], text("a"));
                assert_eq!(arr.elements[1], text("b"));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn dedupe_preserves_first_occurrence_order() {
        let input = Value::Array(Array::from((
            vec![text("x"), text("y"), text("z"), text("x"), text("y")],
            Level::Line,
        )));
        let result = Dedupe.apply(input).unwrap();
        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 3);
                assert_eq!(arr.elements[0], text("x"));
                assert_eq!(arr.elements[1], text("y"));
                assert_eq!(arr.elements[2], text("z"));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn dedupe_numbers() {
        let input = Value::Array(Array::from((
            vec![Value::Number(1.0), Value::Number(2.0), Value::Number(1.0)],
            Level::Line,
        )));
        let result = Dedupe.apply(input).unwrap();
        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 2);
                assert_eq!(arr.elements[0], Value::Number(1.0));
                assert_eq!(arr.elements[1], Value::Number(2.0));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn dedupe_empty_array() {
        let input = Value::Array(Array::from((vec![], Level::Line)));
        let result = Dedupe.apply(input).unwrap();
        match result {
            Value::Array(arr) => {
                assert!(arr.is_empty());
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn dedupe_all_unique() {
        let input = Value::Array(Array::from((
            vec![text("a"), text("b"), text("c")],
            Level::Line,
        )));
        let result = Dedupe.apply(input).unwrap();
        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 3);
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn dedupe_all_same() {
        let input = Value::Array(Array::from((
            vec![text("a"), text("a"), text("a")],
            Level::Line,
        )));
        let result = Dedupe.apply(input).unwrap();
        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 1);
                assert_eq!(arr.elements[0], text("a"));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn dedupe_preserves_level() {
        let input = Value::Array(Array::from((
            vec![text("a"), text("b"), text("a")],
            Level::Word,
        )));
        let result = Dedupe.apply(input).unwrap();
        match result {
            Value::Array(arr) => {
                assert_eq!(arr.level, Level::Word);
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn dedupe_non_array_is_identity() {
        let input = text("hello");
        let result = Dedupe.apply(input).unwrap();
        assert_eq!(result, text("hello"));
    }
}
