//! JSON formatting for interactive and non-interactive output.

use std::io;

use crossterm::style::{Attribute, Color, SetAttribute, SetForegroundColor};

use crate::value::Value;

/// Format JSON preview as lines with depth-based highlighting and width truncation.
pub fn format_json_preview(
    value: &Value,
    depth: usize,
    max_lines: usize,
    max_width: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    match value {
        Value::Array(arr) => {
            let mut ctx = JsonLineCtx::new(max_width);
            ctx.write_punct("[");
            lines.push(ctx.finish());

            for (i, elem) in arr.elements.iter().enumerate() {
                if lines.len() >= max_lines {
                    break;
                }
                let mut ctx = JsonLineCtx::new(max_width);
                ctx.write_str("  ");
                if i == 0 {
                    ctx.write_value(elem, depth == 0, depth);
                } else {
                    ctx.write_value(elem, false, 0);
                }
                if i < arr.elements.len() - 1 {
                    ctx.write_punct(",");
                }
                lines.push(ctx.finish());
            }
            if lines.len() < max_lines {
                let mut ctx = JsonLineCtx::new(max_width);
                ctx.write_punct("]");
                lines.push(ctx.finish());
            }
        }
        _ => {
            let mut ctx = JsonLineCtx::new(max_width);
            ctx.write_value(value, depth == 0, 0);
            lines.push(ctx.finish());
        }
    }
    lines
}

/// Context for building a truncated JSON line.
struct JsonLineCtx {
    buf: String,
    visible_len: usize,
    max_width: usize,
    truncated: bool,
}

impl JsonLineCtx {
    fn new(max_width: usize) -> Self {
        Self {
            buf: String::new(),
            visible_len: 0,
            max_width,
            truncated: false,
        }
    }

    fn finish(mut self) -> String {
        if self.truncated {
            self.buf.push_str("\x1b[0m"); // Reset any active styles
        }
        self.buf
    }

    fn write_str(&mut self, s: &str) {
        if self.truncated {
            return;
        }
        let remaining = self.max_width.saturating_sub(self.visible_len);
        if s.len() <= remaining {
            self.buf.push_str(s);
            self.visible_len += s.len();
        } else if remaining > 3 {
            self.buf.push_str(&s[..remaining - 3]);
            self.buf.push_str("...");
            self.visible_len = self.max_width;
            self.truncated = true;
        } else {
            self.buf.push_str("...");
            self.truncated = true;
        }
    }

    fn write_punct(&mut self, s: &str) {
        use std::fmt::Write;
        write!(&mut self.buf, "{}", SetForegroundColor(Color::White)).unwrap();
        self.write_str(s);
        write!(&mut self.buf, "{}", SetForegroundColor(Color::Reset)).unwrap();
    }

    fn write_value(&mut self, value: &Value, highlight: bool, depth: usize) {
        use std::fmt::Write;
        if self.truncated {
            return;
        }
        if highlight {
            write!(&mut self.buf, "{}", SetAttribute(Attribute::Bold)).unwrap();
            self.write_compact(value);
            write!(&mut self.buf, "{}", SetAttribute(Attribute::NoBold)).unwrap();
        } else if depth > 0 {
            match value {
                Value::Array(arr) if !arr.elements.is_empty() => {
                    self.write_punct("[");
                    for (i, elem) in arr.elements.iter().enumerate() {
                        if self.truncated {
                            break;
                        }
                        if i > 0 {
                            self.write_punct(",");
                        }
                        if i == 0 {
                            self.write_value(elem, depth == 1, depth - 1);
                        } else {
                            self.write_compact(elem);
                        }
                    }
                    self.write_punct("]");
                }
                _ => self.write_compact(value),
            }
        } else {
            self.write_compact(value);
        }
    }

    fn write_compact(&mut self, value: &Value) {
        use std::fmt::Write;
        if self.truncated {
            return;
        }
        match value {
            Value::Text(t) => {
                let escaped = serde_json::to_string(t).unwrap_or_else(|_| format!("{:?}", t));
                write!(&mut self.buf, "{}", SetForegroundColor(Color::Green)).unwrap();
                self.write_str(&escaped);
                write!(&mut self.buf, "{}", SetForegroundColor(Color::Reset)).unwrap();
            }
            Value::Number(n) => {
                write!(&mut self.buf, "{}", SetForegroundColor(Color::Cyan)).unwrap();
                self.write_str(&n.to_string());
                write!(&mut self.buf, "{}", SetForegroundColor(Color::Reset)).unwrap();
            }
            Value::Array(arr) => {
                self.write_punct("[");
                for (i, elem) in arr.elements.iter().enumerate() {
                    if self.truncated {
                        break;
                    }
                    if i > 0 {
                        self.write_punct(",");
                    }
                    self.write_compact(elem);
                }
                self.write_punct("]");
            }
        }
    }
}

/// Write JSON punctuation in white.
fn write_json_punct<W: io::Write>(w: &mut W, s: &str) -> io::Result<()> {
    write!(
        w,
        "{}{}{}",
        SetForegroundColor(Color::White),
        s,
        SetForegroundColor(Color::Reset)
    )
}

/// Write compact JSON with syntax highlighting (but no depth highlight).
fn write_json_compact_highlighted<W: io::Write>(w: &mut W, value: &Value) -> io::Result<()> {
    match value {
        Value::Text(s) => {
            let escaped = serde_json::to_string(s).unwrap_or_else(|_| format!("{:?}", s));
            write!(
                w,
                "{}{}{}",
                SetForegroundColor(Color::Green),
                escaped,
                SetForegroundColor(Color::Reset)
            )
        }
        Value::Number(n) => {
            write!(
                w,
                "{}{}{}",
                SetForegroundColor(Color::Cyan),
                n,
                SetForegroundColor(Color::Reset)
            )
        }
        Value::Array(arr) => {
            write_json_punct(w, "[")?;
            for (i, elem) in arr.elements.iter().enumerate() {
                if i > 0 {
                    write_json_punct(w, ",")?;
                }
                write_json_compact_highlighted(w, elem)?;
            }
            write_json_punct(w, "]")
        }
    }
}

/// Write syntax-highlighted JSON to a writer (non-interactive).
pub fn write_json_highlighted<W: io::Write>(
    w: &mut W,
    value: &Value,
    use_color: bool,
) -> io::Result<()> {
    match value {
        Value::Array(arr) => {
            write!(w, "[")?;
            for (i, elem) in arr.elements.iter().enumerate() {
                write!(w, "\n  ")?;
                write_json_value_noninteractive(w, elem, use_color)?;
                if i < arr.elements.len() - 1 {
                    write!(w, ",")?;
                }
            }
            write!(w, "\n]")?;
        }
        _ => {
            write_json_value_noninteractive(w, value, use_color)?;
        }
    }
    Ok(())
}

/// Write a JSON value for non-interactive output (compact inner arrays).
fn write_json_value_noninteractive<W: io::Write>(
    w: &mut W,
    value: &Value,
    use_color: bool,
) -> io::Result<()> {
    if use_color {
        write_json_compact_highlighted(w, value)
    } else {
        let json =
            serde_json::to_string(value).unwrap_or_else(|e| format!("\"JSON error: {}\"", e));
        write!(w, "{}", json)
    }
}
