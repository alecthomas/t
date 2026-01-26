use crate::error::Result;
use crate::interpreter::Transform;
use crate::value::Value;

pub struct SortDescending;

impl Transform for SortDescending {
    fn apply(&self, value: Value) -> Result<Value> {
        match value {
            Value::Array(mut arr) => {
                arr.sort(true);
                Ok(Value::Array(arr))
            }
            other => Ok(other),
        }
    }

    fn requires_full_input(&self) -> bool {
        true
    }
}

pub struct SortAscending;

impl Transform for SortAscending {
    fn apply(&self, value: Value) -> Result<Value> {
        match value {
            Value::Array(mut arr) => {
                arr.sort(false);
                Ok(Value::Array(arr))
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
    use crate::value::{Array, Level};

    fn text(s: &str) -> Value {
        Value::Text(s.to_string())
    }

    #[test]
    fn sort_descending_numbers() {
        let input = Value::Array(Array::from((
            vec![
                Value::Number(3.0),
                Value::Number(1.0),
                Value::Number(4.0),
                Value::Number(1.0),
                Value::Number(5.0),
            ],
            Level::Line,
        )));
        let result = SortDescending.apply(input).unwrap();
        match result {
            Value::Array(arr) => {
                assert_eq!(arr.elements[0], Value::Number(5.0));
                assert_eq!(arr.elements[1], Value::Number(4.0));
                assert_eq!(arr.elements[2], Value::Number(3.0));
                assert_eq!(arr.elements[3], Value::Number(1.0));
                assert_eq!(arr.elements[4], Value::Number(1.0));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn sort_ascending_numbers() {
        let input = Value::Array(Array::from((
            vec![
                Value::Number(3.0),
                Value::Number(1.0),
                Value::Number(4.0),
                Value::Number(1.0),
                Value::Number(5.0),
            ],
            Level::Line,
        )));
        let result = SortAscending.apply(input).unwrap();
        match result {
            Value::Array(arr) => {
                assert_eq!(arr.elements[0], Value::Number(1.0));
                assert_eq!(arr.elements[1], Value::Number(1.0));
                assert_eq!(arr.elements[2], Value::Number(3.0));
                assert_eq!(arr.elements[3], Value::Number(4.0));
                assert_eq!(arr.elements[4], Value::Number(5.0));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn sort_descending_lexicographic() {
        let input = Value::Array(Array::from((
            vec![
                Value::Array(Array::from((
                    vec![Value::Number(2.0), text("b")],
                    Level::Line,
                ))),
                Value::Array(Array::from((
                    vec![Value::Number(1.0), text("a")],
                    Level::Line,
                ))),
                Value::Array(Array::from((
                    vec![Value::Number(2.0), text("a")],
                    Level::Line,
                ))),
            ],
            Level::Line,
        )));
        let result = SortDescending.apply(input).unwrap();
        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 3);
                match &arr.elements[0] {
                    Value::Array(inner) => {
                        assert_eq!(inner.elements[0], Value::Number(2.0));
                        assert_eq!(inner.elements[1], text("b"));
                    }
                    _ => panic!("expected array"),
                }
                match &arr.elements[1] {
                    Value::Array(inner) => {
                        assert_eq!(inner.elements[0], Value::Number(2.0));
                        assert_eq!(inner.elements[1], text("a"));
                    }
                    _ => panic!("expected array"),
                }
                match &arr.elements[2] {
                    Value::Array(inner) => {
                        assert_eq!(inner.elements[0], Value::Number(1.0));
                        assert_eq!(inner.elements[1], text("a"));
                    }
                    _ => panic!("expected array"),
                }
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn sort_ascending_lexicographic() {
        let input = Value::Array(Array::from((
            vec![
                Value::Array(Array::from((
                    vec![Value::Number(2.0), text("b")],
                    Level::Line,
                ))),
                Value::Array(Array::from((
                    vec![Value::Number(1.0), text("a")],
                    Level::Line,
                ))),
                Value::Array(Array::from((
                    vec![Value::Number(2.0), text("a")],
                    Level::Line,
                ))),
            ],
            Level::Line,
        )));
        let result = SortAscending.apply(input).unwrap();
        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 3);
                match &arr.elements[0] {
                    Value::Array(inner) => {
                        assert_eq!(inner.elements[0], Value::Number(1.0));
                        assert_eq!(inner.elements[1], text("a"));
                    }
                    _ => panic!("expected array"),
                }
                match &arr.elements[1] {
                    Value::Array(inner) => {
                        assert_eq!(inner.elements[0], Value::Number(2.0));
                        assert_eq!(inner.elements[1], text("a"));
                    }
                    _ => panic!("expected array"),
                }
                match &arr.elements[2] {
                    Value::Array(inner) => {
                        assert_eq!(inner.elements[0], Value::Number(2.0));
                        assert_eq!(inner.elements[1], text("b"));
                    }
                    _ => panic!("expected array"),
                }
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn sort_descending_strings() {
        let input = Value::Array(Array::from((
            vec![text("banana"), text("apple"), text("cherry")],
            Level::Line,
        )));
        let result = SortDescending.apply(input).unwrap();
        match result {
            Value::Array(arr) => {
                assert_eq!(arr.elements[0], text("cherry"));
                assert_eq!(arr.elements[1], text("banana"));
                assert_eq!(arr.elements[2], text("apple"));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn sort_ascending_strings() {
        let input = Value::Array(Array::from((
            vec![text("banana"), text("apple"), text("cherry")],
            Level::Line,
        )));
        let result = SortAscending.apply(input).unwrap();
        match result {
            Value::Array(arr) => {
                assert_eq!(arr.elements[0], text("apple"));
                assert_eq!(arr.elements[1], text("banana"));
                assert_eq!(arr.elements[2], text("cherry"));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn sort_descending_non_array_is_identity() {
        let input = text("hello");
        let result = SortDescending.apply(input).unwrap();
        assert_eq!(result, text("hello"));

        let input = Value::Number(42.0);
        let result = SortDescending.apply(input).unwrap();
        assert_eq!(result, Value::Number(42.0));
    }

    #[test]
    fn sort_ascending_non_array_is_identity() {
        let input = text("hello");
        let result = SortAscending.apply(input).unwrap();
        assert_eq!(result, text("hello"));

        let input = Value::Number(42.0);
        let result = SortAscending.apply(input).unwrap();
        assert_eq!(result, Value::Number(42.0));
    }
}
