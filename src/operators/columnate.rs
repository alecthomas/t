use crate::error::Result;
use crate::interpreter::Transform;
use crate::value::{Array, Level, Value};

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
                    return Ok(Value::Array(arr));
                }

                let max_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
                let mut col_widths = vec![0usize; max_cols];
                for row in &rows {
                    for (i, cell) in row.iter().enumerate() {
                        col_widths[i] = col_widths[i].max(cell.chars().count());
                    }
                }

                let elements: Vec<Value> = rows
                    .into_iter()
                    .map(|row| {
                        let last_idx = row.len().saturating_sub(1);
                        let cells: Vec<Value> = row
                            .into_iter()
                            .enumerate()
                            .map(|(i, cell)| {
                                if i == last_idx {
                                    Value::Text(cell)
                                } else {
                                    let width = col_widths.get(i).copied().unwrap_or(0);
                                    let padding = width.saturating_sub(cell.chars().count());
                                    Value::Text(format!("{}{}", cell, " ".repeat(padding)))
                                }
                            })
                            .collect();
                        Value::Array(Array::from((cells, Level::Word)))
                    })
                    .collect();

                Ok(Value::Array(Array::from((elements, arr.level))))
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

    fn row(cells: Vec<&str>) -> Value {
        Value::Array(Array::from((
            cells.into_iter().map(text).collect(),
            Level::Word,
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
        let expected = Value::Array(Array::from((
            vec![
                row(vec!["name ", "age"]),
                row(vec!["alice", "30"]),
                row(vec!["bob  ", "25"]),
            ],
            Level::Line,
        )));
        assert_eq!(result, expected);
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
        let expected = Value::Array(Array::from((
            vec![
                row(vec!["a   ", "bb", "ccc"]),
                row(vec!["dddd", "e ", "ff"]),
            ],
            Level::Line,
        )));
        assert_eq!(result, expected);
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
        let expected = Value::Array(Array::from((
            vec![row(vec!["one", "two", "three"])],
            Level::Line,
        )));
        assert_eq!(result, expected);
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
        let expected = Value::Array(Array::from((
            vec![row(vec!["first"]), row(vec!["second"]), row(vec!["third"])],
            Level::Line,
        )));
        assert_eq!(result, expected);
    }

    #[test]
    fn columnate_empty_array() {
        let input = Value::Array(Array::from((vec![], Level::Line)));
        let result = Columnate.apply(input).unwrap();
        let expected = Value::Array(Array::from((vec![], Level::Line)));
        assert_eq!(result, expected);
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
        let expected = Value::Array(Array::from((
            vec![
                row(vec!["count", "value"]),
                row(vec!["42   ", "foo"]),
                row(vec!["7    ", "bar"]),
            ],
            Level::Line,
        )));
        assert_eq!(result, expected);
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
        let expected = Value::Array(Array::from((
            vec![
                row(vec!["a", "b", "c"]),
                row(vec!["d", "e"]),
                row(vec!["f"]),
            ],
            Level::Line,
        )));
        assert_eq!(result, expected);
    }

    #[test]
    fn columnate_non_array_rows() {
        let input = Value::Array(Array::from((
            vec![text("hello"), text("world")],
            Level::Line,
        )));
        let result = Columnate.apply(input).unwrap();
        let expected = Value::Array(Array::from((
            vec![row(vec!["hello"]), row(vec!["world"])],
            Level::Line,
        )));
        assert_eq!(result, expected);
    }

    #[test]
    fn columnate_non_array_is_identity() {
        let input = text("hello");
        let result = Columnate.apply(input).unwrap();
        assert_eq!(result, text("hello"));
    }
}
