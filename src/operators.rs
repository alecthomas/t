//! Operator implementations for the t language.
//!
//! Each operator is a struct that implements either `Transform` or `Navigate`.

use std::collections::HashMap;

use regex::Regex;

use crate::ast::Selection;
use crate::error::{Error, Result};
use crate::interpreter::{Context, Navigate, Transform};
use crate::value::{Array, Level, Value};

/// Split operator - splits text by semantic level.
///
/// - File -> lines (split on newlines)
/// - Line -> words (split on whitespace)
/// - Word -> chars (split into characters)
pub struct Split;

impl Transform for Split {
    fn apply(&self, value: Value) -> Result<Value> {
        match value {
            Value::Array(mut arr) => {
                arr.elements = arr
                    .elements
                    .into_iter()
                    .map(|v| self.apply(v))
                    .collect::<Result<Vec<_>>>()?;
                Ok(Value::Array(arr))
            }
            Value::Text(s) => {
                // Default to Line level for bare text
                Ok(split_text(&s, Level::Line))
            }
            Value::Number(n) => Ok(Value::Number(n)),
        }
    }
}

/// Split text according to its semantic level.
fn split_text(s: &str, level: Level) -> Value {
    let new_level = level.split_into();
    let elements: Vec<Value> = match level {
        Level::File => s
            .lines()
            .map(|line| Value::Text(line.to_string()))
            .collect(),
        Level::Line => s
            .split_whitespace()
            .map(|word| Value::Text(word.to_string()))
            .collect(),
        Level::Word => s.chars().map(|c| Value::Text(c.to_string())).collect(),
        Level::Char => vec![Value::Text(s.to_string())],
    };
    Value::Array(Array::from((elements, new_level)))
}

/// Join operator - joins arrays by semantic level and flattens.
///
/// - Array of arrays: flatten one level, return array
/// - Array of strings/numbers: join with space, return string
pub struct Join;

impl Transform for Join {
    fn apply(&self, value: Value) -> Result<Value> {
        match value {
            Value::Array(arr) => Ok(join_array(arr)),
            other => Ok(other),
        }
    }
}

/// Join an array according to its contents.
///
/// - Array of arrays: flatten one level, return array
/// - Array of strings/numbers: join with space, return string
fn join_array(arr: Array) -> Value {
    // Check if first element is an array (flatten case)
    let first_is_array = arr
        .elements
        .first()
        .map(|v| matches!(v, Value::Array(_)))
        .unwrap_or(false);

    if first_is_array {
        // Flatten: concatenate child arrays into one
        let mut flattened: Vec<Value> = Vec::new();
        let mut inner_level = arr.level;

        for elem in arr.elements {
            if let Value::Array(inner) = elem {
                inner_level = inner.level;
                flattened.extend(inner.elements);
            }
        }

        Value::Array(Array::from((flattened, inner_level)))
    } else {
        // Join strings/numbers with space
        let parts: Vec<String> = arr
            .elements
            .into_iter()
            .map(|v| match v {
                Value::Text(s) => s,
                Value::Number(n) => n.to_string(),
                Value::Array(inner) => {
                    // Recursively join nested arrays
                    match join_array(inner) {
                        Value::Text(s) => s,
                        Value::Number(n) => n.to_string(),
                        _ => String::new(),
                    }
                }
            })
            .collect();

        Value::Text(parts.join(" "))
    }
}

/// Lowercase operator - converts text to lowercase.
pub struct Lowercase;

impl Transform for Lowercase {
    fn apply(&self, value: Value) -> Result<Value> {
        match value {
            Value::Array(mut arr) => {
                arr.elements = arr
                    .elements
                    .into_iter()
                    .map(|v| self.apply(v))
                    .collect::<Result<Vec<_>>>()?;
                Ok(Value::Array(arr))
            }
            Value::Text(s) => Ok(Value::Text(s.to_lowercase())),
            Value::Number(n) => Ok(Value::Number(n)),
        }
    }
}

/// Trim operator - removes leading and trailing whitespace from text.
pub struct Trim;

impl Transform for Trim {
    fn apply(&self, value: Value) -> Result<Value> {
        match value {
            Value::Array(mut arr) => {
                arr.elements = arr
                    .elements
                    .into_iter()
                    .map(|v| self.apply(v))
                    .collect::<Result<Vec<_>>>()?;
                Ok(Value::Array(arr))
            }
            Value::Text(s) => Ok(Value::Text(s.trim().to_string())),
            Value::Number(n) => Ok(Value::Number(n)),
        }
    }
}

/// Uppercase operator - converts text to uppercase.
pub struct Uppercase;

impl Transform for Uppercase {
    fn apply(&self, value: Value) -> Result<Value> {
        match value {
            Value::Array(mut arr) => {
                arr.elements = arr
                    .elements
                    .into_iter()
                    .map(|v| self.apply(v))
                    .collect::<Result<Vec<_>>>()?;
                Ok(Value::Array(arr))
            }
            Value::Text(s) => Ok(Value::Text(s.to_uppercase())),
            Value::Number(n) => Ok(Value::Number(n)),
        }
    }
}

/// Descend operator - increments depth to operate on nested elements.
pub struct Descend;

impl Navigate for Descend {
    fn apply(&self, ctx: &mut Context) {
        ctx.descend();
    }
}

/// Ascend operator - decrements depth to operate on parent level.
pub struct Ascend;

impl Navigate for Ascend {
    fn apply(&self, ctx: &mut Context) {
        ctx.ascend();
    }
}

/// DeleteEmpty operator - removes empty elements from an array.
///
/// Empty strings and empty arrays are removed.
pub struct DeleteEmpty;

impl Transform for DeleteEmpty {
    fn apply(&self, value: Value) -> Result<Value> {
        match value {
            Value::Array(mut arr) => {
                arr.elements.retain(|v| !v.is_empty());
                Ok(Value::Array(arr))
            }
            other => Ok(other),
        }
    }
}

/// DedupeWithCounts operator - removes duplicates and counts occurrences.
///
/// Returns `[[count, value], ...]` sorted by count descending.
/// For equal counts, preserves the order of first occurrence.
pub struct DedupeWithCounts;

impl Transform for DedupeWithCounts {
    fn apply(&self, value: Value) -> Result<Value> {
        match value {
            Value::Array(arr) => {
                // Count occurrences while tracking insertion order
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

                // Build result: collect (count, order, value) tuples
                let mut result: Vec<(usize, usize, Value)> = values
                    .into_iter()
                    .map(|v| {
                        let key = value_to_key(&v);
                        let (count, order) = counts[&key];
                        (count, order, v)
                    })
                    .collect();

                // Sort by count descending, then by insertion order ascending for ties
                result.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));

                // Convert to [[count, value], ...]
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
}

/// Convert a Value to a string key for deduplication.
/// Uses a format that distinguishes between types.
fn value_to_key(value: &Value) -> String {
    match value {
        Value::Text(s) => format!("T:{}", s),
        Value::Number(n) => format!("N:{}", n),
        Value::Array(arr) => {
            let inner: Vec<String> = arr.elements.iter().map(value_to_key).collect();
            format!("A:[{}]", inner.join(","))
        }
    }
}

/// Sum operator - sums all numeric values, recursing through arrays.
///
/// Returns a single number representing the sum.
pub struct Sum;

impl Transform for Sum {
    fn apply(&self, value: Value) -> Result<Value> {
        Ok(Value::Number(sum_recursive(&value)))
    }
}

/// Recursively sum all numeric values in a value.
fn sum_recursive(value: &Value) -> f64 {
    match value {
        Value::Array(arr) => arr.elements.iter().map(sum_recursive).sum(),
        Value::Number(n) => *n,
        Value::Text(s) => s.parse::<f64>().unwrap_or(0.0),
    }
}

/// Count operator - returns the number of elements in an array.
///
/// This is a structural operator that counts top-level elements only.
pub struct Count;

impl Transform for Count {
    fn apply(&self, value: Value) -> Result<Value> {
        match value {
            Value::Array(arr) => Ok(Value::Number(arr.len() as f64)),
            Value::Text(s) => Ok(Value::Number(s.chars().count() as f64)),
            Value::Number(_) => Ok(Value::Number(0.0)),
        }
    }
}

/// SortDescending operator - sorts an array in descending order.
///
/// For arrays of arrays, sorts lexicographically.
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
}

/// SortAscending operator - sorts an array in ascending order.
///
/// For arrays of arrays, sorts lexicographically.
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
}

/// Filter operator - keeps or removes elements matching a regex pattern.
pub struct Filter {
    pattern: Regex,
    negate: bool,
}

impl Filter {
    /// Create a new Filter operator with the given pattern and negate flag.
    pub fn new(pattern: Regex, negate: bool) -> Self {
        Self { pattern, negate }
    }
}

impl Transform for Filter {
    fn apply(&self, value: Value) -> Result<Value> {
        match value {
            Value::Array(arr) => {
                let filtered: Vec<Value> = arr
                    .elements
                    .into_iter()
                    .filter(|elem| {
                        let text = match elem {
                            Value::Text(s) => s.clone(),
                            Value::Number(n) => n.to_string(),
                            Value::Array(inner) => inner.to_string(),
                        };
                        let matches = self.pattern.is_match(&text);
                        if self.negate { !matches } else { matches }
                    })
                    .collect();
                Ok(Value::Array(Array::from((filtered, arr.level))))
            }
            other => Ok(other),
        }
    }
}

/// Select operator - selects elements by index, slice, or multi-select.
pub struct Select {
    selection: Selection,
}

impl Select {
    /// Create a new Select operator with the given selection.
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

/// Select elements from an array based on a selection.
fn select_from_array(arr: Array, selection: &Selection) -> Result<Value> {
    let len = arr.len() as i64;

    // Single index returns the element directly
    if selection.items.len() == 1
        && let crate::ast::SelectItem::Index(idx) = &selection.items[0]
    {
        let actual = normalize_index(*idx, len);
        return arr
            .elements
            .into_iter()
            .nth(actual as usize)
            .ok_or_else(|| Error::runtime(format!("index {} out of bounds", idx)));
    }

    // Multiple items or slices return an array
    let mut result = Vec::new();
    for item in &selection.items {
        match item {
            crate::ast::SelectItem::Index(idx) => {
                let actual = normalize_index(*idx, len);
                if actual >= 0
                    && actual < len
                    && let Some(v) = arr.elements.get(actual as usize)
                {
                    result.push(v.deep_copy());
                }
            }
            crate::ast::SelectItem::Slice(slice) => {
                let indices = compute_slice_indices(slice, len);
                for i in indices {
                    if let Some(v) = arr.elements.get(i) {
                        result.push(v.deep_copy());
                    }
                }
            }
        }
    }

    Ok(Value::Array(Array::from((result, arr.level))))
}

/// Select characters from a string based on a selection.
fn select_from_string(s: &str, selection: &Selection) -> Result<Value> {
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len() as i64;

    // Single index returns a single-char string
    if selection.items.len() == 1
        && let crate::ast::SelectItem::Index(idx) = &selection.items[0]
    {
        let actual = normalize_index(*idx, len);
        return chars
            .get(actual as usize)
            .map(|c| Value::Text(c.to_string()))
            .ok_or_else(|| Error::runtime(format!("index {} out of bounds", idx)));
    }

    // Multiple items or slices return a string
    let mut result = String::new();
    for item in &selection.items {
        match item {
            crate::ast::SelectItem::Index(idx) => {
                let actual = normalize_index(*idx, len);
                if actual >= 0
                    && actual < len
                    && let Some(c) = chars.get(actual as usize)
                {
                    result.push(*c);
                }
            }
            crate::ast::SelectItem::Slice(slice) => {
                let indices = compute_slice_indices(slice, len);
                for i in indices {
                    if let Some(c) = chars.get(i) {
                        result.push(*c);
                    }
                }
            }
        }
    }

    Ok(Value::Text(result))
}

/// Normalize a possibly-negative index to a positive index.
fn normalize_index(idx: i64, len: i64) -> i64 {
    if idx < 0 { idx + len } else { idx }
}

/// Compute the indices for a slice.
fn compute_slice_indices(slice: &crate::ast::Slice, len: i64) -> Vec<usize> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{SelectItem, Slice};

    fn text(s: &str) -> Value {
        Value::Text(s.to_string())
    }

    fn line_array(lines: &[&str]) -> Value {
        Value::Array(Array::from((
            lines.iter().map(|s| text(s)).collect(),
            Level::Line,
        )))
    }

    fn word_array(words: &[&str]) -> Value {
        Value::Array(Array::from((
            words.iter().map(|s| text(s)).collect(),
            Level::Word,
        )))
    }

    // Split tests

    #[test]
    fn split_line_into_words() {
        let result = Split.apply(text("hello world")).unwrap();
        match result {
            Value::Array(arr) => {
                assert_eq!(arr.level, Level::Word);
                assert_eq!(arr.len(), 2);
                assert_eq!(arr.elements[0], text("hello"));
                assert_eq!(arr.elements[1], text("world"));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn split_array_of_lines() {
        let input = line_array(&["hello world", "foo bar baz"]);
        let result = Split.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 2);
                match &arr.elements[0] {
                    Value::Array(inner) => {
                        assert_eq!(inner.len(), 2);
                        assert_eq!(inner.elements[0], text("hello"));
                    }
                    _ => panic!("expected inner array"),
                }
                match &arr.elements[1] {
                    Value::Array(inner) => {
                        assert_eq!(inner.len(), 3);
                    }
                    _ => panic!("expected inner array"),
                }
            }
            _ => panic!("expected array"),
        }
    }

    // Join tests

    #[test]
    fn join_strings_with_space() {
        let input = word_array(&["hello", "world"]);
        let result = Join.apply(input).unwrap();
        assert_eq!(result, text("hello world"));
    }

    #[test]
    fn join_single_string() {
        let input = word_array(&["hello"]);
        let result = Join.apply(input).unwrap();
        assert_eq!(result, text("hello"));
    }

    #[test]
    fn join_empty_array_of_strings() {
        let input = Value::Array(Array::from((vec![], Level::Word)));
        let result = Join.apply(input).unwrap();
        assert_eq!(result, text(""));
    }

    #[test]
    fn join_flattens_array_of_arrays() {
        let inner1 = word_array(&["hello", "world"]);
        let inner2 = word_array(&["foo", "bar"]);
        let outer = Value::Array(Array::from((vec![inner1, inner2], Level::Line)));

        let result = Join.apply(outer).unwrap();
        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 4);
                assert_eq!(arr.elements[0], text("hello"));
                assert_eq!(arr.elements[1], text("world"));
                assert_eq!(arr.elements[2], text("foo"));
                assert_eq!(arr.elements[3], text("bar"));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn join_flattens_single_inner_array() {
        let inner = word_array(&["a", "b", "c"]);
        let outer = Value::Array(Array::from((vec![inner], Level::Line)));

        let result = Join.apply(outer).unwrap();
        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 3);
                assert_eq!(arr.elements[0], text("a"));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn join_flattens_empty_inner_arrays() {
        let inner1 = Value::Array(Array::from((vec![], Level::Word)));
        let inner2 = word_array(&["a"]);
        let inner3 = Value::Array(Array::from((vec![], Level::Word)));
        let outer = Value::Array(Array::from((vec![inner1, inner2, inner3], Level::Line)));

        let result = Join.apply(outer).unwrap();
        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 1);
                assert_eq!(arr.elements[0], text("a"));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn join_preserves_inner_level() {
        let inner1 = Value::Array(Array::from((vec![text("a")], Level::Char)));
        let inner2 = Value::Array(Array::from((vec![text("b")], Level::Char)));
        let outer = Value::Array(Array::from((vec![inner1, inner2], Level::Word)));

        let result = Join.apply(outer).unwrap();
        match result {
            Value::Array(arr) => {
                assert_eq!(arr.level, Level::Char);
                assert_eq!(arr.len(), 2);
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn join_numbers_with_space() {
        let input = Value::Array(Array::from((
            vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)],
            Level::Word,
        )));
        let result = Join.apply(input).unwrap();
        assert_eq!(result, text("1 2 3"));
    }

    #[test]
    fn join_non_array_is_identity() {
        let input = text("hello");
        let result = Join.apply(input).unwrap();
        assert_eq!(result, text("hello"));
    }

    // Lowercase tests

    #[test]
    fn lowercase_text() {
        let result = Lowercase.apply(text("HELLO World")).unwrap();
        assert_eq!(result, text("hello world"));
    }

    #[test]
    fn lowercase_array() {
        let input = line_array(&["HELLO", "WORLD"]);
        let result = Lowercase.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.elements[0], text("hello"));
                assert_eq!(arr.elements[1], text("world"));
            }
            _ => panic!("expected array"),
        }
    }

    // Uppercase tests

    #[test]
    fn uppercase_text() {
        let result = Uppercase.apply(text("hello World")).unwrap();
        assert_eq!(result, text("HELLO WORLD"));
    }

    // Trim tests

    #[test]
    fn trim_text() {
        let result = Trim.apply(text("  hello  ")).unwrap();
        assert_eq!(result, text("hello"));
    }

    #[test]
    fn trim_text_with_tabs_and_newlines() {
        let result = Trim.apply(text("\thello\n")).unwrap();
        assert_eq!(result, text("hello"));
    }

    #[test]
    fn trim_array() {
        let input = line_array(&["  hello  ", "\tworld\n"]);
        let result = Trim.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.elements[0], text("hello"));
                assert_eq!(arr.elements[1], text("world"));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn trim_preserves_numbers() {
        let result = Trim.apply(Value::Number(42.0)).unwrap();
        assert_eq!(result, Value::Number(42.0));
    }

    // Select tests

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

    // DeleteEmpty tests

    #[test]
    fn delete_empty_removes_empty_strings() {
        let input = Value::Array(Array::from((
            vec![text("a"), text(""), text("b"), text(""), text("c")],
            Level::Line,
        )));
        let result = DeleteEmpty.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 3);
                assert_eq!(arr.elements[0], text("a"));
                assert_eq!(arr.elements[1], text("b"));
                assert_eq!(arr.elements[2], text("c"));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn delete_empty_removes_empty_arrays() {
        let inner1 = word_array(&["a", "b"]);
        let inner2 = Value::Array(Array::from((vec![], Level::Word)));
        let inner3 = word_array(&["c"]);
        let input = Value::Array(Array::from((vec![inner1, inner2, inner3], Level::Line)));

        let result = DeleteEmpty.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 2);
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn delete_empty_keeps_numbers() {
        let input = Value::Array(Array::from((
            vec![Value::Number(0.0), text(""), Value::Number(1.0)],
            Level::Line,
        )));
        let result = DeleteEmpty.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 2);
                assert_eq!(arr.elements[0], Value::Number(0.0));
                assert_eq!(arr.elements[1], Value::Number(1.0));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn delete_empty_on_all_empty() {
        let input = Value::Array(Array::from((vec![text(""), text("")], Level::Line)));
        let result = DeleteEmpty.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert!(arr.is_empty());
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn delete_empty_non_array_is_identity() {
        let input = text("hello");
        let result = DeleteEmpty.apply(input).unwrap();
        assert_eq!(result, text("hello"));
    }

    // DedupeWithCounts tests

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
                // First entry: [3, "a"]
                match &arr.elements[0] {
                    Value::Array(inner) => {
                        assert_eq!(inner.len(), 2);
                        assert_eq!(inner.elements[0], Value::Number(3.0));
                        assert_eq!(inner.elements[1], text("a"));
                    }
                    _ => panic!("expected inner array"),
                }
                // Second entry: [2, "b"]
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
                // All have count 1, should preserve insertion order
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

    // Sum tests

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

    // Count tests

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
        // Counts top-level elements only, not recursive
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
                // Should be [[2, "b"], [2, "a"], [1, "a"]]
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
                // Should be [[1, "a"], [2, "a"], [2, "b"]]
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

    // Filter tests

    #[test]
    fn filter_keep_matching() {
        let input = Value::Array(Array::from((
            vec![text("apple"), text("banana"), text("apricot")],
            Level::Line,
        )));
        let filter = Filter::new(Regex::new("^a").unwrap(), false);
        let result = filter.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 2);
                assert_eq!(arr.elements[0], text("apple"));
                assert_eq!(arr.elements[1], text("apricot"));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn filter_remove_matching() {
        let input = Value::Array(Array::from((
            vec![text("apple"), text("banana"), text("apricot")],
            Level::Line,
        )));
        let filter = Filter::new(Regex::new("^a").unwrap(), true);
        let result = filter.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 1);
                assert_eq!(arr.elements[0], text("banana"));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn filter_no_matches() {
        let input = Value::Array(Array::from((
            vec![text("apple"), text("banana"), text("cherry")],
            Level::Line,
        )));
        let filter = Filter::new(Regex::new("^z").unwrap(), false);
        let result = filter.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert!(arr.is_empty());
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn filter_all_match() {
        let input = Value::Array(Array::from((
            vec![text("apple"), text("apricot"), text("avocado")],
            Level::Line,
        )));
        let filter = Filter::new(Regex::new("^a").unwrap(), false);
        let result = filter.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 3);
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn filter_with_numbers() {
        let input = Value::Array(Array::from((
            vec![
                Value::Number(10.0),
                Value::Number(20.0),
                Value::Number(100.0),
            ],
            Level::Line,
        )));
        let filter = Filter::new(Regex::new("^1").unwrap(), false);
        let result = filter.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 2);
                assert_eq!(arr.elements[0], Value::Number(10.0));
                assert_eq!(arr.elements[1], Value::Number(100.0));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn filter_preserves_level() {
        let input = Value::Array(Array::from((
            vec![text("apple"), text("banana")],
            Level::Word,
        )));
        let filter = Filter::new(Regex::new("a").unwrap(), false);
        let result = filter.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.level, Level::Word);
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn filter_non_array_is_identity() {
        let input = text("hello");
        let filter = Filter::new(Regex::new("e").unwrap(), false);
        let result = filter.apply(input).unwrap();
        assert_eq!(result, text("hello"));
    }

    #[test]
    fn filter_empty_array() {
        let input = Value::Array(Array::from((vec![], Level::Line)));
        let filter = Filter::new(Regex::new("a").unwrap(), false);
        let result = filter.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert!(arr.is_empty());
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn filter_regex_contains() {
        let input = Value::Array(Array::from((
            vec![text("ERROR: fail"), text("INFO: ok"), text("ERROR: crash")],
            Level::Line,
        )));
        let filter = Filter::new(Regex::new("ERROR").unwrap(), false);
        let result = filter.apply(input).unwrap();

        match result {
            Value::Array(arr) => {
                assert_eq!(arr.len(), 2);
                assert_eq!(arr.elements[0], text("ERROR: fail"));
                assert_eq!(arr.elements[1], text("ERROR: crash"));
            }
            _ => panic!("expected array"),
        }
    }
}
