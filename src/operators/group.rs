use std::collections::HashMap;

use crate::ast::Selection;
use crate::error::{Error, Result};
use crate::interpreter::Transform;
use crate::value::{Array, Value};

use super::dedupe::value_to_key;
use super::select::selection_indices;

pub struct GroupBy {
    selection: Selection,
}

impl GroupBy {
    pub fn new(selection: Selection) -> Self {
        Self { selection }
    }
}

impl Transform for GroupBy {
    fn apply(&self, value: Value) -> Result<Value> {
        match value {
            Value::Array(arr) => {
                let mut groups: Vec<(Value, Vec<Value>)> = Vec::new();
                let mut key_indices: HashMap<String, usize> = HashMap::new();

                for elem in arr.elements {
                    let key = extract_key(&elem, &self.selection)?;
                    let key_str = value_to_key(&key);

                    if let Some(&idx) = key_indices.get(&key_str) {
                        groups[idx].1.push(elem);
                    } else {
                        let idx = groups.len();
                        key_indices.insert(key_str, idx);
                        groups.push((key, vec![elem]));
                    }
                }

                let elements: Vec<Value> = groups
                    .into_iter()
                    .map(|(key, elems)| {
                        Value::Array(Array::from((
                            vec![key, Value::Array(Array::from((elems, arr.level)))],
                            arr.level,
                        )))
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

fn extract_key(elem: &Value, selection: &Selection) -> Result<Value> {
    match elem {
        Value::Array(arr) => {
            let len = arr.len() as i64;
            let indices = selection_indices(selection, len);

            if indices.len() == 1 {
                return arr
                    .elements
                    .get(indices[0])
                    .map(|v| v.deep_copy())
                    .ok_or_else(|| Error::runtime("index out of bounds"));
            }

            let result: Vec<Value> = indices
                .iter()
                .filter_map(|&i| arr.elements.get(i).map(|v| v.deep_copy()))
                .collect();
            Ok(Value::Array(Array::from((result, arr.level))))
        }
        other => Ok(other.deep_copy()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::SelectItem;
    use crate::value::Level;

    fn text(s: &str) -> Value {
        Value::Text(s.to_string())
    }

    #[test]
    fn group_by_single_index() {
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
        let group_by = GroupBy::new(Selection {
            items: vec![SelectItem::Index(0)],
        });
        let result = group_by.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 2);

                match &arr.elements[0] {
                    Value::Array(group) => {
                        assert_eq!(group.len(), 2);
                        assert_eq!(group.elements[0], text("a"));
                        match &group.elements[1] {
                            Value::Array(elems) => {
                                assert_eq!(elems.len(), 2);
                            }
                            _ => panic!("expected array of elements"),
                        }
                    }
                    _ => panic!("expected group array"),
                }

                match &arr.elements[1] {
                    Value::Array(group) => {
                        assert_eq!(group.len(), 2);
                        assert_eq!(group.elements[0], text("b"));
                        match &group.elements[1] {
                            Value::Array(elems) => {
                                assert_eq!(elems.len(), 1);
                            }
                            _ => panic!("expected array of elements"),
                        }
                    }
                    _ => panic!("expected group array"),
                }
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn group_by_preserves_order() {
        let input = Value::Array(Array::from((
            vec![
                Value::Array(Array::from((
                    vec![text("x"), Value::Number(1.0)],
                    Level::Word,
                ))),
                Value::Array(Array::from((
                    vec![text("y"), Value::Number(2.0)],
                    Level::Word,
                ))),
                Value::Array(Array::from((
                    vec![text("z"), Value::Number(3.0)],
                    Level::Word,
                ))),
            ],
            Level::Line,
        )));
        let group_by = GroupBy::new(Selection {
            items: vec![SelectItem::Index(0)],
        });
        let result = group_by.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 3);
                match &arr.elements[0] {
                    Value::Array(g) => assert_eq!(g.elements[0], text("x")),
                    _ => panic!("expected array"),
                }
                match &arr.elements[1] {
                    Value::Array(g) => assert_eq!(g.elements[0], text("y")),
                    _ => panic!("expected array"),
                }
                match &arr.elements[2] {
                    Value::Array(g) => assert_eq!(g.elements[0], text("z")),
                    _ => panic!("expected array"),
                }
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn group_by_composite_key() {
        let input = Value::Array(Array::from((
            vec![
                Value::Array(Array::from((
                    vec![text("a"), text("x"), Value::Number(1.0)],
                    Level::Word,
                ))),
                Value::Array(Array::from((
                    vec![text("a"), text("y"), Value::Number(2.0)],
                    Level::Word,
                ))),
                Value::Array(Array::from((
                    vec![text("a"), text("x"), Value::Number(3.0)],
                    Level::Word,
                ))),
            ],
            Level::Line,
        )));
        let group_by = GroupBy::new(Selection {
            items: vec![SelectItem::Index(0), SelectItem::Index(1)],
        });
        let result = group_by.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 2);

                match &arr.elements[0] {
                    Value::Array(group) => {
                        match &group.elements[0] {
                            Value::Array(key) => {
                                assert_eq!(key.len(), 2);
                                assert_eq!(key.elements[0], text("a"));
                                assert_eq!(key.elements[1], text("x"));
                            }
                            _ => panic!("expected composite key"),
                        }
                        match &group.elements[1] {
                            Value::Array(elems) => {
                                assert_eq!(elems.len(), 2);
                            }
                            _ => panic!("expected array"),
                        }
                    }
                    _ => panic!("expected group"),
                }
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn group_by_empty_array() {
        let input = Value::Array(Array::from((vec![], Level::Line)));
        let group_by = GroupBy::new(Selection {
            items: vec![SelectItem::Index(0)],
        });
        let result = group_by.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert!(arr.is_empty());
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn group_by_non_array_is_identity() {
        let input = text("hello");
        let group_by = GroupBy::new(Selection {
            items: vec![SelectItem::Index(0)],
        });
        let result = group_by.apply(input).unwrap();
        assert_eq!(result, text("hello"));
    }

    #[test]
    fn group_by_negative_index() {
        let input = Value::Array(Array::from((
            vec![
                Value::Array(Array::from((vec![text("x"), text("a")], Level::Word))),
                Value::Array(Array::from((vec![text("y"), text("b")], Level::Word))),
                Value::Array(Array::from((vec![text("z"), text("a")], Level::Word))),
            ],
            Level::Line,
        )));
        let group_by = GroupBy::new(Selection {
            items: vec![SelectItem::Index(-1)],
        });
        let result = group_by.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 2);
                match &arr.elements[0] {
                    Value::Array(g) => {
                        assert_eq!(g.elements[0], text("a"));
                        match &g.elements[1] {
                            Value::Array(elems) => assert_eq!(elems.len(), 2),
                            _ => panic!("expected array"),
                        }
                    }
                    _ => panic!("expected array"),
                }
            }
            _ => panic!("expected array"),
        }
    }
}
