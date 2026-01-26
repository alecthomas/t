//! Data model for the t language.
//!
//! The core types are:
//! - `Value`: A tagged union of Array, Text, or Number

#![allow(dead_code)] // Module not yet used in main
//! - `Array`: An array with semantic level for split/join behavior
//! - `Level`: Semantic level determining how arrays split and join

use std::cmp::Ordering;
use std::fmt;
use std::fs;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

use serde::Serialize;
use serde::ser::{SerializeSeq, Serializer};

/// Semantic level of array contents - determines split/join behavior.
///
/// | Level | `s` splits into | `j` joins with |
/// |-------|-----------------|----------------|
/// | File  | lines           | newline        |
/// | Line  | words           | space          |
/// | Word  | chars           | nothing        |
/// | Char  | no-op           | nothing        |
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Level {
    File,
    #[default]
    Line,
    Word,
    Char,
}

impl Level {
    /// Returns the level of elements produced when splitting at this level.
    pub fn split_into(self) -> Level {
        match self {
            Level::File => Level::Line,
            Level::Line => Level::Word,
            Level::Word => Level::Char,
            Level::Char => Level::Char,
        }
    }

    /// Returns the parent level (what contains elements of this level).
    pub fn parent(self) -> Level {
        match self {
            Level::File => Level::File,
            Level::Line => Level::File,
            Level::Word => Level::Line,
            Level::Char => Level::Word,
        }
    }

    /// Returns the delimiter used when joining elements of this level.
    ///
    /// E.g., joining words (Level::Word) uses space (because words form a line).
    /// Joining lines (Level::Line) uses newline (because lines form a file).
    pub fn join_delimiter(self) -> &'static str {
        match self {
            Level::File => "\n",
            Level::Line => "\n",
            Level::Word => " ",
            Level::Char => "",
        }
    }
}

/// A value in the t language.
#[derive(Debug, PartialEq)]
pub enum Value {
    Array(Array),
    Text(String),
    Number(f64),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Text(s) => write!(f, "{}", s),
            Value::Number(n) => write!(f, "{}", n),
            Value::Array(arr) => write!(f, "{}", arr),
        }
    }
}

impl fmt::Display for Array {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let delimiter = self.level.join_delimiter();
        let mut first = true;
        for elem in &self.elements {
            if !first {
                write!(f, "{}", delimiter)?;
            }
            first = false;
            write!(f, "{}", elem)?;
        }
        Ok(())
    }
}

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Value::Text(s) => serializer.serialize_str(s),
            Value::Number(n) => serializer.serialize_f64(*n),
            Value::Array(arr) => arr.serialize(serializer),
        }
    }
}

impl Serialize for Array {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.elements.len()))?;
        for elem in &self.elements {
            seq.serialize_element(elem)?;
        }
        seq.end()
    }
}

impl Value {
    /// Create an explicit deep copy of this value.
    ///
    /// This is intentionally not `Clone` to prevent accidental copying.
    /// Use this only when a true copy is needed (e.g., duplicate selection indices).
    pub fn deep_copy(&self) -> Self {
        match self {
            Value::Array(arr) => Value::Array(arr.deep_copy()),
            Value::Text(s) => Value::Text(s.clone()),
            Value::Number(n) => Value::Number(*n),
        }
    }

    /// Replace this value with another.
    pub fn replace(&mut self, new: Value) {
        *self = new;
    }

    /// Check if a value is considered "empty".
    ///
    /// - Empty strings are empty
    /// - Empty arrays are empty
    /// - Numbers are never empty (including 0)
    pub fn is_empty(&self) -> bool {
        match self {
            Value::Text(s) => s.is_empty(),
            Value::Array(arr) => arr.is_empty(),
            Value::Number(_) => false,
        }
    }

    /// Returns a type tag for ordering: Number < Text < Array.
    fn type_order(&self) -> u8 {
        match self {
            Value::Number(_) => 0,
            Value::Text(_) => 1,
            Value::Array(_) => 2,
        }
    }
}

impl Eq for Value {}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Value {
    /// Compare values for sorting. Order: Number < Text < Array.
    /// Arrays compare lexicographically (Python-style).
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => a.total_cmp(b),
            (Value::Text(a), Value::Text(b)) => a.cmp(b),
            (Value::Array(a), Value::Array(b)) => a.cmp(b),
            _ => self.type_order().cmp(&other.type_order()),
        }
    }
}

/// An array with semantic level.
#[derive(Debug, PartialEq, Eq)]
pub struct Array {
    pub level: Level,
    pub elements: Vec<Value>,
}

impl Array {
    /// Create an explicit deep copy of this array.
    ///
    /// This is intentionally not `Clone` to prevent accidental copying.
    /// Use this only when a true copy is needed (e.g., duplicate selection indices).
    pub fn deep_copy(&self) -> Self {
        Self {
            level: self.level,
            elements: self.elements.iter().map(|v| v.deep_copy()).collect(),
        }
    }

    /// Create a truncated deep copy with at most `limit` elements.
    pub fn truncated_copy(&self, limit: usize) -> Self {
        Self {
            level: self.level,
            elements: self
                .elements
                .iter()
                .take(limit)
                .map(|v| v.deep_copy())
                .collect(),
        }
    }

    /// Create an empty array with the given level.
    pub fn new(level: Level) -> Self {
        Self {
            level,
            elements: Vec::new(),
        }
    }

    /// Load input from stdin.
    /// - `Level::File`: entire stdin as one Text element
    /// - `Level::Line`: stdin split into lines
    pub fn from_stdin(level: Level) -> io::Result<Self> {
        let stdin = io::stdin();
        Self::from_reader(stdin.lock(), level)
    }

    /// Load input from files.
    /// - `Level::File`: each file as one Text element
    /// - `Level::Line`: all files split into lines, concatenated
    pub fn from_files(paths: &[impl AsRef<Path>], level: Level) -> io::Result<Self> {
        let mut elements = Vec::new();

        for path in paths {
            let file = fs::File::open(path)?;
            let reader = BufReader::new(file);

            match level {
                Level::File => {
                    let mut contents = String::new();
                    BufReader::new(fs::File::open(path)?).read_to_string(&mut contents)?;
                    if contents.ends_with('\n') {
                        contents.pop();
                        if contents.ends_with('\r') {
                            contents.pop();
                        }
                    }
                    elements.push(Value::Text(contents));
                }
                _ => {
                    for line in reader.lines() {
                        elements.push(Value::Text(line?));
                    }
                }
            }
        }

        Ok(Self { level, elements })
    }

    /// Load from a reader.
    fn from_reader<R: BufRead>(reader: R, level: Level) -> io::Result<Self> {
        let mut elements = Vec::new();

        match level {
            Level::File => {
                let mut contents = String::new();
                let mut reader = reader;
                reader.read_to_string(&mut contents)?;
                if contents.ends_with('\n') {
                    contents.pop();
                    if contents.ends_with('\r') {
                        contents.pop();
                    }
                }
                elements.push(Value::Text(contents));
            }
            _ => {
                for line in reader.lines() {
                    elements.push(Value::Text(line?));
                }
            }
        }

        Ok(Self { level, elements })
    }

    /// Get element by index. Negative indices count from end.
    pub fn get(&self, index: i64) -> Option<&Value> {
        let len = self.elements.len() as i64;
        let actual = if index < 0 { index + len } else { index };
        if actual < 0 || actual >= len {
            None
        } else {
            self.elements.get(actual as usize)
        }
    }

    /// Returns the number of elements.
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Returns true if the array is empty.
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Sort the array in place.
    pub fn sort(&mut self, descending: bool) {
        if descending {
            self.elements.sort_by(|a, b| b.cmp(a));
        } else {
            self.elements.sort();
        }
    }

    /// Returns an iterator over the elements.
    pub fn iter(&self) -> impl Iterator<Item = &Value> {
        self.elements.iter()
    }
}

impl From<(Vec<Value>, Level)> for Array {
    fn from((elements, level): (Vec<Value>, Level)) -> Self {
        Self { level, elements }
    }
}

impl PartialOrd for Array {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Array {
    /// Compare arrays lexicographically (Python-style).
    /// Shorter arrays are less than longer arrays if they're a prefix.
    fn cmp(&self, other: &Self) -> Ordering {
        self.elements.cmp(&other.elements)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_split_into() {
        assert_eq!(Level::File.split_into(), Level::Line);
        assert_eq!(Level::Line.split_into(), Level::Word);
        assert_eq!(Level::Word.split_into(), Level::Char);
        assert_eq!(Level::Char.split_into(), Level::Char);
    }

    #[test]
    fn test_level_join_delimiter() {
        assert_eq!(Level::File.join_delimiter(), "\n");
        assert_eq!(Level::Line.join_delimiter(), "\n");
        assert_eq!(Level::Word.join_delimiter(), " ");
        assert_eq!(Level::Char.join_delimiter(), "");
    }

    #[test]
    fn test_level_parent() {
        assert_eq!(Level::File.parent(), Level::File);
        assert_eq!(Level::Line.parent(), Level::File);
        assert_eq!(Level::Word.parent(), Level::Line);
        assert_eq!(Level::Char.parent(), Level::Word);
    }

    #[test]
    fn test_value_ordering() {
        let num = Value::Number(1.0);
        let text = Value::Text("hello".to_string());
        let arr = Value::Array(Array::from((vec![], Level::Line)));

        assert!(num < text);
        assert!(text < arr);
        assert!(num < arr);
    }

    #[test]
    fn test_number_comparison() {
        let a = Value::Number(1.0);
        let b = Value::Number(2.0);
        let c = Value::Number(1.0);

        assert!(a < b);
        assert_eq!(a, c);
    }

    #[test]
    fn test_text_comparison() {
        let a = Value::Text("bar".to_string());
        let b = Value::Text("foo".to_string());

        assert!(a < b);
    }

    #[test]
    fn test_value_replace() {
        let cases = [
            (Value::Text("a".into()), Value::Text("b".into())),
            (Value::Text("a".into()), Value::Number(1.0)),
            (
                Value::Text("a".into()),
                Value::Array(Array::new(Level::Line)),
            ),
            (Value::Number(1.0), Value::Text("a".into())),
            (Value::Number(1.0), Value::Number(2.0)),
            (Value::Number(1.0), Value::Array(Array::new(Level::Line))),
            (
                Value::Array(Array::new(Level::Line)),
                Value::Text("a".into()),
            ),
            (Value::Array(Array::new(Level::Line)), Value::Number(1.0)),
            (
                Value::Array(Array::new(Level::Line)),
                Value::Array(Array::new(Level::Word)),
            ),
        ];

        for (mut from, to) in cases {
            let to_copy = to.deep_copy();
            from.replace(to);
            assert_eq!(from, to_copy);
        }
    }

    #[test]
    fn test_array_lexicographic_sort() {
        let mut arr = Array::from((
            vec![
                Value::Array(Array::from((
                    vec![Value::Number(1.0), Value::Text("foo".to_string())],
                    Level::Line,
                ))),
                Value::Array(Array::from((
                    vec![Value::Number(1.0), Value::Text("bar".to_string())],
                    Level::Line,
                ))),
                Value::Array(Array::from((
                    vec![Value::Number(3.0), Value::Text("bar".to_string())],
                    Level::Line,
                ))),
            ],
            Level::Line,
        ));

        arr.sort(false);

        let elements = &arr.elements;
        if let Value::Array(first) = &elements[0] {
            assert_eq!(first.elements[0], Value::Number(1.0));
            assert_eq!(first.elements[1], Value::Text("bar".to_string()));
        } else {
            panic!("Expected array");
        }
        if let Value::Array(second) = &elements[1] {
            assert_eq!(second.elements[0], Value::Number(1.0));
            assert_eq!(second.elements[1], Value::Text("foo".to_string()));
        } else {
            panic!("Expected array");
        }
        if let Value::Array(third) = &elements[2] {
            assert_eq!(third.elements[0], Value::Number(3.0));
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_array_from_tuple() {
        let arr = Array::from((
            vec![Value::Text("a".to_string()), Value::Text("b".to_string())],
            Level::Line,
        ));

        assert_eq!(arr.len(), 2);
        assert_eq!(arr.elements[0], Value::Text("a".to_string()));
    }

    #[test]
    fn test_array_get_positive_index() {
        let arr = Array::from((
            vec![
                Value::Text("a".to_string()),
                Value::Text("b".to_string()),
                Value::Text("c".to_string()),
            ],
            Level::Line,
        ));

        assert_eq!(arr.get(0), Some(&Value::Text("a".to_string())));
        assert_eq!(arr.get(1), Some(&Value::Text("b".to_string())));
        assert_eq!(arr.get(2), Some(&Value::Text("c".to_string())));
        assert_eq!(arr.get(3), None);
    }

    #[test]
    fn test_array_get_negative_index() {
        let arr = Array::from((
            vec![
                Value::Text("a".to_string()),
                Value::Text("b".to_string()),
                Value::Text("c".to_string()),
            ],
            Level::Line,
        ));

        assert_eq!(arr.get(-1), Some(&Value::Text("c".to_string())));
        assert_eq!(arr.get(-2), Some(&Value::Text("b".to_string())));
        assert_eq!(arr.get(-3), Some(&Value::Text("a".to_string())));
        assert_eq!(arr.get(-4), None);
    }

    #[test]
    fn test_array_iter() {
        let arr = Array::from((
            vec![Value::Text("a".to_string()), Value::Text("b".to_string())],
            Level::Line,
        ));

        let collected: Vec<_> = arr.iter().map(|v| v.deep_copy()).collect();
        assert_eq!(collected.len(), 2);
        assert_eq!(collected[0], Value::Text("a".to_string()));
        assert_eq!(collected[1], Value::Text("b".to_string()));
    }

    #[test]
    fn test_array_sort_descending() {
        let mut arr = Array::from((
            vec![Value::Number(1.0), Value::Number(3.0), Value::Number(2.0)],
            Level::Line,
        ));

        arr.sort(true);

        assert_eq!(arr.elements[0], Value::Number(3.0));
        assert_eq!(arr.elements[1], Value::Number(2.0));
        assert_eq!(arr.elements[2], Value::Number(1.0));
    }

    #[test]
    fn test_array_sort_ascending() {
        let mut arr = Array::from((
            vec![Value::Number(3.0), Value::Number(1.0), Value::Number(2.0)],
            Level::Line,
        ));

        arr.sort(false);

        assert_eq!(arr.elements[0], Value::Number(1.0));
        assert_eq!(arr.elements[1], Value::Number(2.0));
        assert_eq!(arr.elements[2], Value::Number(3.0));
    }

    #[test]
    fn test_array_new() {
        let arr = Array::new(Level::Line);
        assert!(arr.is_empty());
        assert_eq!(arr.level, Level::Line);
    }

    #[test]
    fn test_array_is_empty() {
        let empty = Array::new(Level::Line);
        assert!(empty.is_empty());

        let non_empty = Array::from((vec![Value::Text("a".to_string())], Level::Line));
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn test_array_from_reader_line_level() {
        let input = "line1\nline2\nline3";
        let reader = std::io::BufReader::new(input.as_bytes());
        let arr = Array::from_reader(reader, Level::Line).unwrap();

        assert_eq!(arr.level, Level::Line);
        assert_eq!(arr.len(), 3);
        assert_eq!(arr.elements[0], Value::Text("line1".to_string()));
        assert_eq!(arr.elements[1], Value::Text("line2".to_string()));
        assert_eq!(arr.elements[2], Value::Text("line3".to_string()));
    }

    #[test]
    fn test_array_from_reader_file_level() {
        let input = "line1\nline2\nline3";
        let reader = std::io::BufReader::new(input.as_bytes());
        let arr = Array::from_reader(reader, Level::File).unwrap();

        assert_eq!(arr.level, Level::File);
        assert_eq!(arr.len(), 1);
        assert_eq!(
            arr.elements[0],
            Value::Text("line1\nline2\nline3".to_string())
        );
    }

    #[test]
    fn test_array_from_reader_strips_trailing_newline() {
        let input = "content\n";
        let reader = std::io::BufReader::new(input.as_bytes());
        let arr = Array::from_reader(reader, Level::File).unwrap();

        assert_eq!(arr.elements[0], Value::Text("content".to_string()));
    }

    #[test]
    fn test_array_from_files() {
        let dir = std::env::temp_dir();
        let path1 = dir.join("t_test_file1.txt");
        let path2 = dir.join("t_test_file2.txt");

        std::fs::write(&path1, "file1 line1\nfile1 line2").unwrap();
        std::fs::write(&path2, "file2 line1").unwrap();

        // Line level: all lines concatenated
        let arr = Array::from_files(&[&path1, &path2], Level::Line).unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr.elements[0], Value::Text("file1 line1".to_string()));
        assert_eq!(arr.elements[1], Value::Text("file1 line2".to_string()));
        assert_eq!(arr.elements[2], Value::Text("file2 line1".to_string()));

        // File level: each file as one element
        let arr = Array::from_files(&[&path1, &path2], Level::File).unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(
            arr.elements[0],
            Value::Text("file1 line1\nfile1 line2".to_string())
        );
        assert_eq!(arr.elements[1], Value::Text("file2 line1".to_string()));

        std::fs::remove_file(&path1).unwrap();
        std::fs::remove_file(&path2).unwrap();
    }
}
