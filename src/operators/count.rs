use crate::error::Result;
use crate::interpreter::Transform;
use crate::value::Value;

pub struct Count;

impl Transform for Count {
    fn apply(&self, value: Value) -> Result<Value> {
        match value {
            Value::Array(arr) => Ok(Value::Number(arr.len() as f64)),
            Value::Text(s) => Ok(Value::Number(s.chars().count() as f64)),
            Value::Number(_) => Ok(Value::Number(0.0)),
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
    fn count_array() {
        let input = Value::Array(Array::from((
            vec![text("a"), text("b"), text("c")],
            Level::Line,
        )));
        let result = Count.apply(input).unwrap();
        assert_eq!(result, Value::Number(3.0));
    }

    #[test]
    fn count_empty_array() {
        let input = Value::Array(Array::from((vec![], Level::Line)));
        let result = Count.apply(input).unwrap();
        assert_eq!(result, Value::Number(0.0));
    }

    #[test]
    fn count_nested_arrays() {
        let inner1 = Value::Array(Array::from((vec![text("a"), text("b")], Level::Word)));
        let inner2 = Value::Array(Array::from((vec![text("c")], Level::Word)));
        let input = Value::Array(Array::from((vec![inner1, inner2], Level::Line)));
        let result = Count.apply(input).unwrap();
        assert_eq!(result, Value::Number(2.0));
    }

    #[test]
    fn count_text_returns_length() {
        let input = text("hello");
        let result = Count.apply(input).unwrap();
        assert_eq!(result, Value::Number(5.0));
    }

    #[test]
    fn count_number_returns_zero() {
        let input = Value::Number(42.0);
        let result = Count.apply(input).unwrap();
        assert_eq!(result, Value::Number(0.0));
    }
}
