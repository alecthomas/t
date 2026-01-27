//! Text formatting for interactive output.

use crate::value::Value;

/// Count the number of output lines a value would produce when displayed.
pub fn count_output_lines(value: &Value) -> usize {
    match value {
        Value::Array(arr) => arr.len(),
        Value::Text(s) => s.lines().count().max(1),
        Value::Number(_) => 1,
    }
}

/// Format a value as text with depth highlighting marker.
/// At depth 0, the first line is the "current unit".
/// At depth 1+, the first element within each line is highlighted.
pub fn format_text_with_depth(value: &Value, depth: usize) -> Vec<String> {
    match value {
        Value::Array(arr) => {
            let delimiter = arr.level.join_delimiter();
            arr.elements
                .iter()
                .enumerate()
                .map(|(i, elem)| {
                    if depth > 0 && i == 0 {
                        format_text_element_highlighted(elem, depth - 1)
                    } else {
                        format_text_element(elem, delimiter)
                    }
                })
                .collect()
        }
        Value::Text(s) => s.lines().map(|l| l.to_string()).collect(),
        Value::Number(n) => vec![n.to_string()],
    }
}

/// Format a single element as text, joining sub-elements with the given delimiter.
fn format_text_element(value: &Value, _delimiter: &str) -> String {
    format!("{}", value)
}

/// Format a text element with the first sub-element highlighted (for depth > 0).
fn format_text_element_highlighted(value: &Value, remaining_depth: usize) -> String {
    match value {
        Value::Array(arr) if !arr.elements.is_empty() => {
            let delimiter = arr.level.join_delimiter();
            let mut parts: Vec<String> = Vec::new();
            for (i, elem) in arr.elements.iter().enumerate() {
                if i == 0 {
                    if remaining_depth > 0 {
                        parts.push(format_text_element_highlighted(elem, remaining_depth - 1));
                    } else {
                        // This is the element to highlight - wrap with ANSI bold
                        parts.push(format!(
                            "\x1b[1m{}\x1b[0m",
                            format_text_element(elem, delimiter)
                        ));
                    }
                } else {
                    parts.push(format_text_element(elem, delimiter));
                }
            }
            parts.join(delimiter)
        }
        _ => format!("{}", value),
    }
}
