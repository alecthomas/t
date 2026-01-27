use crate::ast::{SelectItem, Selection, Slice};
use crate::error::{Error, Result};
use crate::interpreter::Transform;
use crate::value::{Array, Value};

pub struct Select {
    selection: Selection,
}

impl Select {
    pub fn new(selection: Selection) -> Self {
        Self { selection }
    }
}

impl Transform for Select {
    fn apply(&self, value: Value) -> Result<Value> {
        match value {
            Value::Array(arr) => select_from_array(arr, &self.selection),
            Value::Text(s) => select_from_string(&s, &self.selection),
            Value::Number(_) => Err(Error::runtime("cannot select from number")),
        }
    }
}

fn select_from_array(arr: Array, selection: &Selection) -> Result<Value> {
    let len = arr.len() as i64;
    let indices = selection_indices(selection, len);

    if indices.len() == 1 {
        let idx = indices[0];
        let mut elements = arr.elements;
        if idx < elements.len() {
            return Ok(elements.swap_remove(idx));
        }
        return Err(Error::runtime("index out of bounds"));
    }

    let mut result: Vec<Value> = Vec::with_capacity(indices.len());
    for &i in &indices {
        if let Some(v) = arr.elements.get(i) {
            result.push(v.deep_copy());
        }
    }

    Ok(Value::Array(Array::from((result, arr.level))))
}

fn select_from_string(s: &str, selection: &Selection) -> Result<Value> {
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len() as i64;
    let indices = selection_indices(selection, len);

    if indices.len() == 1 {
        return chars
            .get(indices[0])
            .map(|c| Value::Text(c.to_string()))
            .ok_or_else(|| Error::runtime("index out of bounds"));
    }

    let result: String = indices.iter().filter_map(|&i| chars.get(i)).collect();

    Ok(Value::Text(result))
}

pub fn normalize_index(idx: i64, len: i64) -> i64 {
    if idx < 0 { idx + len } else { idx }
}

pub fn compute_slice_indices(slice: &Slice, len: i64) -> Vec<usize> {
    let step = slice.step.unwrap_or(1);
    if step == 0 {
        return Vec::new();
    }

    let (default_start, default_end) = if step > 0 {
        (0, len)
    } else {
        (len - 1, -len - 1)
    };

    let start = normalize_index(slice.start.unwrap_or(default_start), len);
    let end = normalize_index(slice.end.unwrap_or(default_end), len);

    let mut indices = Vec::new();
    let mut i = start;

    if step > 0 {
        while i < end && i < len {
            if i >= 0 {
                indices.push(i as usize);
            }
            i += step;
        }
    } else {
        while i > end && i >= 0 {
            if i < len {
                indices.push(i as usize);
            }
            i += step;
        }
    }

    indices
}

pub fn selection_indices(selection: &Selection, len: i64) -> Vec<usize> {
    let mut indices = Vec::new();
    for item in &selection.items {
        match item {
            SelectItem::Index(idx) => {
                let actual = normalize_index(*idx, len);
                if actual >= 0 && actual < len {
                    indices.push(actual as usize);
                }
            }
            SelectItem::Slice(slice) => {
                indices.extend(compute_slice_indices(slice, len));
            }
        }
    }
    indices
}

pub fn apply_to_selected<F>(arr: Array, selection: &Selection, transform: F) -> Result<Value>
where
    F: Fn(Value) -> Result<Value>,
{
    let len = arr.len() as i64;
    let selected: std::collections::HashSet<usize> =
        selection_indices(selection, len).into_iter().collect();

    let elements: Result<Vec<Value>> = arr
        .elements
        .into_iter()
        .enumerate()
        .map(|(i, v)| {
            if selected.contains(&i) {
                transform(v)
            } else {
                Ok(v)
            }
        })
        .collect();

    Ok(Value::Array(Array::from((elements?, arr.level))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Level;

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
    fn select_single_index() {
        let input = line_array(&["a", "b", "c"]);
        let sel = Selection {
            items: vec![SelectItem::Index(1)],
        };
        let result = Select::new(sel).apply(input).unwrap();
        assert_eq!(result, text("b"));
    }

    #[test]
    fn select_negative_index() {
        let input = line_array(&["a", "b", "c"]);
        let sel = Selection {
            items: vec![SelectItem::Index(-1)],
        };
        let result = Select::new(sel).apply(input).unwrap();
        assert_eq!(result, text("c"));
    }

    #[test]
    fn select_slice() {
        let input = line_array(&["a", "b", "c", "d"]);
        let sel = Selection {
            items: vec![SelectItem::Slice(Slice {
                start: Some(1),
                end: Some(3),
                step: None,
            })],
        };
        let result = Select::new(sel).apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 2);
                assert_eq!(arr.elements[0], text("b"));
                assert_eq!(arr.elements[1], text("c"));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn select_slice_from_start() {
        let input = line_array(&["a", "b", "c"]);
        let sel = Selection {
            items: vec![SelectItem::Slice(Slice {
                start: None,
                end: Some(2),
                step: None,
            })],
        };
        let result = Select::new(sel).apply(input).unwrap();

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
    fn select_slice_to_end() {
        let input = line_array(&["a", "b", "c"]);
        let sel = Selection {
            items: vec![SelectItem::Slice(Slice {
                start: Some(1),
                end: None,
                step: None,
            })],
        };
        let result = Select::new(sel).apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 2);
                assert_eq!(arr.elements[0], text("b"));
                assert_eq!(arr.elements[1], text("c"));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn select_reverse() {
        let input = line_array(&["a", "b", "c"]);
        let sel = Selection {
            items: vec![SelectItem::Slice(Slice {
                start: None,
                end: None,
                step: Some(-1),
            })],
        };
        let result = Select::new(sel).apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 3);
                assert_eq!(arr.elements[0], text("c"));
                assert_eq!(arr.elements[1], text("b"));
                assert_eq!(arr.elements[2], text("a"));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn select_multiple_indices() {
        let input = line_array(&["a", "b", "c", "d"]);
        let sel = Selection {
            items: vec![SelectItem::Index(0), SelectItem::Index(2)],
        };
        let result = Select::new(sel).apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 2);
                assert_eq!(arr.elements[0], text("a"));
                assert_eq!(arr.elements[1], text("c"));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn select_stride() {
        let input = line_array(&["a", "b", "c", "d", "e"]);
        let sel = Selection {
            items: vec![SelectItem::Slice(Slice {
                start: Some(0),
                end: None,
                step: Some(2),
            })],
        };
        let result = Select::new(sel).apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 3);
                assert_eq!(arr.elements[0], text("a"));
                assert_eq!(arr.elements[1], text("c"));
                assert_eq!(arr.elements[2], text("e"));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn select_string_single_char() {
        let input = text("hello");
        let sel = Selection {
            items: vec![SelectItem::Index(1)],
        };
        let result = Select::new(sel).apply(input).unwrap();
        assert_eq!(result, text("e"));
    }

    #[test]
    fn select_string_negative_index() {
        let input = text("hello");
        let sel = Selection {
            items: vec![SelectItem::Index(-1)],
        };
        let result = Select::new(sel).apply(input).unwrap();
        assert_eq!(result, text("o"));
    }

    #[test]
    fn select_string_slice() {
        let input = text("hello");
        let sel = Selection {
            items: vec![SelectItem::Slice(Slice {
                start: Some(1),
                end: Some(4),
                step: None,
            })],
        };
        let result = Select::new(sel).apply(input).unwrap();
        assert_eq!(result, text("ell"));
    }

    #[test]
    fn select_string_reverse() {
        let input = text("hello");
        let sel = Selection {
            items: vec![SelectItem::Slice(Slice {
                start: None,
                end: None,
                step: Some(-1),
            })],
        };
        let result = Select::new(sel).apply(input).unwrap();
        assert_eq!(result, text("olleh"));
    }
}
