use crate::error::Result;
use crate::interpreter::Transform;
use crate::value::Value;

pub struct Sum;

impl Transform for Sum {
    fn apply(&self, value: Value) -> Result<Value> {
        Ok(Value::Number(sum_recursive(&value)))
    }

    fn requires_full_input(&self) -> bool {
        true
    }
}

fn sum_recursive(value: &Value) -> f64 {
    match value {
        Value::Array(arr) => arr.elements.iter().map(sum_recursive).sum(),
        Value::Number(n) => *n,
        Value::Text(s) => s.parse::<f64>().unwrap_or(0.0),
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
    fn sum_numbers() {
        let input = Value::Array(Array::from((
            vec![
                Value::Number(1.0),
                Value::Number(2.0),
                Value::Number(3.0),
                Value::Number(4.0),
            ],
            Level::Line,
        )));
        let result = Sum.apply(input).unwrap();
        assert_eq!(result, Value::Number(10.0));
    }

    #[test]
    fn sum_numeric_strings() {
        let input = Value::Array(Array::from((
            vec![text("1"), text("2"), text("3")],
            Level::Line,
        )));
        let result = Sum.apply(input).unwrap();
        assert_eq!(result, Value::Number(6.0));
    }

    #[test]
    fn sum_mixed_types() {
        let input = Value::Array(Array::from((
            vec![
                Value::Number(1.0),
                text("2"),
                text("hello"),
                Value::Number(3.0),
            ],
            Level::Line,
        )));
        let result = Sum.apply(input).unwrap();
        assert_eq!(result, Value::Number(6.0));
    }

    #[test]
    fn sum_empty_array() {
        let input = Value::Array(Array::from((vec![], Level::Line)));
        let result = Sum.apply(input).unwrap();
        assert_eq!(result, Value::Number(0.0));
    }

    #[test]
    fn sum_single_number() {
        let input = Value::Number(42.0);
        let result = Sum.apply(input).unwrap();
        assert_eq!(result, Value::Number(42.0));
    }

    #[test]
    fn sum_numeric_text() {
        let input = text("42");
        let result = Sum.apply(input).unwrap();
        assert_eq!(result, Value::Number(42.0));
    }

    #[test]
    fn sum_non_numeric_text() {
        let input = text("hello");
        let result = Sum.apply(input).unwrap();
        assert_eq!(result, Value::Number(0.0));
    }

    #[test]
    fn sum_nested_arrays() {
        let inner1 = Value::Array(Array::from((vec![text("1"), text("2")], Level::Word)));
        let inner2 = Value::Array(Array::from((vec![text("3"), text("4")], Level::Word)));
        let input = Value::Array(Array::from((vec![inner1, inner2], Level::Line)));
        let result = Sum.apply(input).unwrap();
        assert_eq!(result, Value::Number(10.0));
    }
}
