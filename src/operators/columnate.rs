use crate::error::Result;
use crate::interpreter::Transform;
use crate::value::Value;

pub struct Columnate;

impl Transform for Columnate {
    fn apply(&self, value: Value) -> Result<Value> {
        match value {
            Value::Array(arr) => {
                let rows: Vec<Vec<String>> = arr
                    .elements
                    .iter()
                    .map(|row| match row {
                        Value::Array(inner) => {
                            inner.elements.iter().map(|v| v.to_string()).collect()
                        }
                        other => vec![other.to_string()],
                    })
                    .collect();

                if rows.is_empty() {
                    return Ok(Value::Text(String::new()));
                }

                let max_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
                let mut col_widths = vec![0usize; max_cols];
                for row in &rows {
                    for (i, cell) in row.iter().enumerate() {
                        col_widths[i] = col_widths[i].max(cell.chars().count());
                    }
                }

                let lines: Vec<String> = rows
                    .into_iter()
                    .map(|row| {
                        row.into_iter()
                            .enumerate()
                            .map(|(i, cell)| {
                                let width = col_widths.get(i).copied().unwrap_or(0);
                                let padding = width.saturating_sub(cell.chars().count());
                                format!("{}{}", cell, " ".repeat(padding))
                            })
                            .collect::<Vec<_>>()
                            .join(" ")
                            .trim_end()
                            .to_string()
                    })
                    .collect();

                Ok(Value::Text(lines.join("\n")))
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

    fn line_array(lines: &[&str]) -> Value {
        Value::Array(Array::from((
            lines.iter().map(|s| text(s)).collect(),
            Level::Line,
        )))
    }

    #[test]
    fn columnate_basic() {
        let input = Value::Array(Array::from((
            vec![
                Value::Array(Array::from((vec![text("name"), text("age")], Level::Word))),
                Value::Array(Array::from((vec![text("alice"), text("30")], Level::Word))),
                Value::Array(Array::from((vec![text("bob"), text("25")], Level::Word))),
            ],
            Level::Line,
        )));
        let result = Columnate.apply(input).unwrap();
        assert_eq!(result, text("name  age\nalice 30\nbob   25"));
    }

    #[test]
    fn columnate_varying_widths() {
        let input = Value::Array(Array::from((
            vec![
                Value::Array(Array::from((
                    vec![text("a"), text("bb"), text("ccc")],
                    Level::Word,
                ))),
                Value::Array(Array::from((
                    vec![text("dddd"), text("e"), text("ff")],
                    Level::Word,
                ))),
            ],
            Level::Line,
        )));
        let result = Columnate.apply(input).unwrap();
        assert_eq!(result, text("a    bb ccc\ndddd e  ff"));
    }

    #[test]
    fn columnate_single_row() {
        let input = Value::Array(Array::from((
            vec![Value::Array(Array::from((
                vec![text("one"), text("two"), text("three")],
                Level::Word,
            )))],
            Level::Line,
        )));
        let result = Columnate.apply(input).unwrap();
        assert_eq!(result, text("one two three"));
    }

    #[test]
    fn columnate_single_column() {
        let input = Value::Array(Array::from((
            vec![
                Value::Array(Array::from((vec![text("first")], Level::Word))),
                Value::Array(Array::from((vec![text("second")], Level::Word))),
                Value::Array(Array::from((vec![text("third")], Level::Word))),
            ],
            Level::Line,
        )));
        let result = Columnate.apply(input).unwrap();
        assert_eq!(result, text("first\nsecond\nthird"));
    }

    #[test]
    fn columnate_empty_array() {
        let input = Value::Array(Array::from((vec![], Level::Line)));
        let result = Columnate.apply(input).unwrap();
        assert_eq!(result, text(""));
    }

    #[test]
    fn columnate_with_numbers() {
        let input = Value::Array(Array::from((
            vec![
                Value::Array(Array::from((
                    vec![text("count"), text("value")],
                    Level::Word,
                ))),
                Value::Array(Array::from((
                    vec![Value::Number(42.0), text("foo")],
                    Level::Word,
                ))),
                Value::Array(Array::from((
                    vec![Value::Number(7.0), text("bar")],
                    Level::Word,
                ))),
            ],
            Level::Line,
        )));
        let result = Columnate.apply(input).unwrap();
        assert_eq!(result, text("count value\n42    foo\n7     bar"));
    }

    #[test]
    fn columnate_uneven_rows() {
        let input = Value::Array(Array::from((
            vec![
                Value::Array(Array::from((
                    vec![text("a"), text("b"), text("c")],
                    Level::Word,
                ))),
                Value::Array(Array::from((vec![text("d"), text("e")], Level::Word))),
                Value::Array(Array::from((vec![text("f")], Level::Word))),
            ],
            Level::Line,
        )));
        let result = Columnate.apply(input).unwrap();
        assert_eq!(result, text("a b c\nd e\nf"));
    }

    #[test]
    fn columnate_non_array_rows() {
        let input = Value::Array(Array::from((
            vec![text("hello"), text("world")],
            Level::Line,
        )));
        let result = Columnate.apply(input).unwrap();
        assert_eq!(result, text("hello\nworld"));
    }

    #[test]
    fn columnate_non_array_is_identity() {
        let input = text("hello");
        let result = Columnate.apply(input).unwrap();
        assert_eq!(result, text("hello"));
    }
}
