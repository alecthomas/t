use crate::ast::Selection;
use crate::error::Result;
use crate::interpreter::Transform;
use crate::value::Value;

use super::select::apply_to_selected;

pub struct ToNumber;

impl Transform for ToNumber {
    fn apply(&self, value: Value) -> Result<Value> {
        match value {
            Value::Array(mut arr) => {
                arr.elements = arr
                    .elements
                    .into_iter()
                    .map(|v| self.apply(v))
                    .collect::<Result<_>>()?;
                Ok(Value::Array(arr))
            }
            Value::Text(s) => Ok(s
                .parse::<f64>()
                .map(Value::Number)
                .unwrap_or(Value::Text(s))),
            Value::Number(n) => Ok(Value::Number(n)),
        }
    }
}

pub struct ToNumberSelected {
    selection: Selection,
}

impl ToNumberSelected {
    pub fn new(selection: Selection) -> Self {
        Self { selection }
    }
}

impl Transform for ToNumberSelected {
    fn apply(&self, value: Value) -> Result<Value> {
        match value {
            Value::Array(arr) => apply_to_selected(arr, &self.selection, |v| ToNumber.apply(v)),
            other => Ok(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{SelectItem, Selection, Slice};
    use crate::value::{Array, Level};

    fn text(s: &str) -> Value {
        Value::Text(s.to_string())
    }

    fn line_array(lines: &[&str]) -> Value {
        Value::Array(Array::from((
            lines.iter().map(|s| text(s)).collect(),
            Level::Line,
        )))
    }

    #[test]
    fn to_number_integer() {
        let input = text("42");
        let result = ToNumber.apply(input).unwrap();
        assert_eq!(result, Value::Number(42.0));
    }

    #[test]
    fn to_number_float() {
        let input = text("3.14");
        let result = ToNumber.apply(input).unwrap();
        assert_eq!(result, Value::Number(3.14));
    }

    #[test]
    fn to_number_negative() {
        let input = text("-123");
        let result = ToNumber.apply(input).unwrap();
        assert_eq!(result, Value::Number(-123.0));
    }

    #[test]
    fn to_number_non_numeric() {
        let input = text("hello");
        let result = ToNumber.apply(input).unwrap();
        assert_eq!(result, text("hello"));
    }

    #[test]
    fn to_number_empty_string() {
        let input = text("");
        let result = ToNumber.apply(input).unwrap();
        assert_eq!(result, text(""));
    }

    #[test]
    fn to_number_preserves_number() {
        let input = Value::Number(42.0);
        let result = ToNumber.apply(input).unwrap();
        assert_eq!(result, Value::Number(42.0));
    }

    #[test]
    fn to_number_array() {
        let input = line_array(&["1", "2", "3"]);
        let result = ToNumber.apply(input).unwrap();
        match result {
            Value::Array(arr) => {
                assert_eq!(arr.elements[0], Value::Number(1.0));
                assert_eq!(arr.elements[1], Value::Number(2.0));
                assert_eq!(arr.elements[2], Value::Number(3.0));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn to_number_array_mixed() {
        let input = line_array(&["1", "hello", "3"]);
        let result = ToNumber.apply(input).unwrap();
        match result {
            Value::Array(arr) => {
                assert_eq!(arr.elements.len(), 3);
                assert_eq!(arr.elements[0], Value::Number(1.0));
                assert_eq!(arr.elements[1], text("hello"));
                assert_eq!(arr.elements[2], Value::Number(3.0));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn to_number_selected_single_index() {
        let input = line_array(&["1", "2", "3"]);
        let sel = Selection {
            items: vec![SelectItem::Index(0)],
        };
        let result = ToNumberSelected::new(sel).apply(input).unwrap();
        match result {
            Value::Array(arr) => {
                assert_eq!(arr.elements[0], Value::Number(1.0));
                assert_eq!(arr.elements[1], text("2"));
                assert_eq!(arr.elements[2], text("3"));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn to_number_selected_slice() {
        let input = line_array(&["1", "2", "3"]);
        let sel = Selection {
            items: vec![SelectItem::Slice(Slice {
                start: Some(1),
                end: None,
                step: None,
            })],
        };
        let result = ToNumberSelected::new(sel).apply(input).unwrap();
        match result {
            Value::Array(arr) => {
                assert_eq!(arr.elements[0], text("1"));
                assert_eq!(arr.elements[1], Value::Number(2.0));
                assert_eq!(arr.elements[2], Value::Number(3.0));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn to_number_selected_multi_index() {
        let input = line_array(&["1", "2", "3", "4"]);
        let sel = Selection {
            items: vec![SelectItem::Index(0), SelectItem::Index(2)],
        };
        let result = ToNumberSelected::new(sel).apply(input).unwrap();
        match result {
            Value::Array(arr) => {
                assert_eq!(arr.elements[0], Value::Number(1.0));
                assert_eq!(arr.elements[1], text("2"));
                assert_eq!(arr.elements[2], Value::Number(3.0));
                assert_eq!(arr.elements[3], text("4"));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn to_number_selected_negative_index() {
        let input = line_array(&["1", "2", "3"]);
        let sel = Selection {
            items: vec![SelectItem::Index(-1)],
        };
        let result = ToNumberSelected::new(sel).apply(input).unwrap();
        match result {
            Value::Array(arr) => {
                assert_eq!(arr.elements[0], text("1"));
                assert_eq!(arr.elements[1], text("2"));
                assert_eq!(arr.elements[2], Value::Number(3.0));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn to_number_selected_non_array_is_identity() {
        let input = text("hello");
        let sel = Selection {
            items: vec![SelectItem::Index(0)],
        };
        let result = ToNumberSelected::new(sel).apply(input).unwrap();
        assert_eq!(result, text("hello"));
    }
}
