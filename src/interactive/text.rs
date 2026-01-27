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
/// Stops after producing `max_lines` lines.
pub fn format_text_with_depth(
    value: &Value,
    depth: usize,
    max_lines: usize,
    max_width: usize,
) -> Vec<String> {
    match value {
        Value::Array(arr) => {
            let mut lines = Vec::with_capacity(max_lines.min(arr.elements.len()));
            for (i, elem) in arr.elements.iter().enumerate() {
                if lines.len() >= max_lines {
                    break;
                }
                let line = if depth > 0 && i == 0 {
                    format_text_element_highlighted(elem, depth - 1, max_width)
                } else {
                    format_text_element(elem, max_width)
                };
                lines.push(line);
            }
            lines
        }
        Value::Text(s) => s
            .lines()
            .take(max_lines)
            .map(|l| truncate_line(l, max_width))
            .collect(),
        Value::Number(n) => vec![truncate_line(&n.to_string(), max_width)],
    }
}

/// Truncate a line to fit within max_width, adding "..." if truncated.
fn truncate_line(line: &str, max_width: usize) -> String {
    if line.len() <= max_width {
        line.to_string()
    } else if max_width > 3 {
        format!("{}...", &line[..max_width - 3])
    } else {
        line[..max_width].to_string()
    }
}

/// Format a single element as text, truncating to max_width.
fn format_text_element(value: &Value, max_width: usize) -> String {
    truncate_line(&format!("{}", value), max_width)
}

/// Format a text element with the first sub-element highlighted (for depth > 0).
/// Uses a streaming approach to avoid formatting beyond max_width.
fn format_text_element_highlighted(
    value: &Value,
    remaining_depth: usize,
    max_width: usize,
) -> String {
    match value {
        Value::Array(arr) if !arr.elements.is_empty() => {
            let delimiter = arr.level.join_delimiter();
            let mut result = String::new();
            for (i, elem) in arr.elements.iter().enumerate() {
                if i > 0 {
                    result.push_str(delimiter);
                }
                // Check if we've exceeded max_width (accounting for "...")
                if result.len() >= max_width.saturating_sub(3) {
                    result.push_str("...");
                    return truncate_line(&result, max_width);
                }
                if i == 0 {
                    if remaining_depth > 0 {
                        let remaining = max_width.saturating_sub(result.len());
                        result.push_str(&format_text_element_highlighted(
                            elem,
                            remaining_depth - 1,
                            remaining,
                        ));
                    } else {
                        // This is the element to highlight - wrap with ANSI bold
                        result.push_str("\x1b[1m");
                        result.push_str(&format!("{}", elem));
                        result.push_str("\x1b[0m");
                    }
                } else {
                    result.push_str(&format!("{}", elem));
                }
            }
            truncate_line(&result, max_width)
        }
        _ => truncate_line(&format!("{}", value), max_width),
    }
}
