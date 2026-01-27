//! Interactive mode for live previewing programmes.

use std::io::{self, Write};

use anyhow::{Context, Result};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
    terminal::{self, ClearType},
};

use crate::ast;
use crate::interpreter;
use crate::parser;
use crate::value::{Array, Value};

const MIN_PREVIEW_LINES: usize = 10;
/// Batch sizes for adaptive preview execution.
const PREVIEW_BATCH_SIZES: &[usize] = &[100, 500, 2000, usize::MAX];

enum HelpLine {
    Heading(&'static str),
    Row(&'static str, &'static str, &'static str, &'static str),
    Single(&'static str, &'static str),
}

const OPERATOR_HELP: &[HelpLine] = &[
    HelpLine::Heading("Operators:"),
    HelpLine::Row("s", "split on whitespace", "S<d>", "split on delimiter"),
    HelpLine::Row("j", "join with level sep", "J<d>", "join with delimiter"),
    HelpLine::Row("l", "lowercase", "L<sel>", "lowercase selected"),
    HelpLine::Row("u", "uppercase", "U<sel>", "uppercase selected"),
    HelpLine::Row("t", "trim whitespace", "T<sel>", "trim selected"),
    HelpLine::Row("n", "to number", "N<sel>", "to number selected"),
    HelpLine::Row(
        "r/<p>/<r>/",
        "replace pattern",
        "r<sel>/<p>/<r>/",
        "replace in selected",
    ),
    HelpLine::Row("/<pat>/", "filter keep", "!/<pat>/", "filter remove"),
    HelpLine::Row("d", "dedupe", "D<sel>", "dedupe on selected"),
    HelpLine::Row("o", "sort descending", "O", "sort ascending"),
    HelpLine::Row("x", "delete empty", "g<sel>", "group by"),
    HelpLine::Row("#", "count", "+", "sum"),
    HelpLine::Row("c", "columnate", "p<sel>", "partition"),
    HelpLine::Row("@", "descend", "^", "ascend"),
    HelpLine::Single("<sel>", "select (e.g. 0, 1:3, ::2)"),
];

const INTERACTIVE_KEYS: &[(&str, &str)] = &[
    ("Enter", "Commit"),
    ("^C/Esc", "Cancel"),
    ("^J", "JSON"),
    ("^H", "Help"),
];

const OP_WIDTH: usize = 16;
const DESC_WIDTH: usize = 21;

/// Generate plain text help for CLI --help.
pub fn help_text() -> String {
    let mut lines = Vec::new();
    for help_line in OPERATOR_HELP {
        match help_line {
            HelpLine::Heading(text) => lines.push(text.to_string()),
            HelpLine::Row(op1, desc1, op2, desc2) => {
                lines.push(format!(
                    "  {:<OP_WIDTH$}{:<DESC_WIDTH$}{:<OP_WIDTH$}{}",
                    op1, desc1, op2, desc2
                ));
            }
            HelpLine::Single(op, desc) => {
                lines.push(format!("  {:<OP_WIDTH$}{}", op, desc));
            }
        }
    }
    lines.join("\n")
}

pub struct InteractiveMode {
    input: Array,
    programme: String,
    cursor: usize,
    json_output: bool,
    show_help: bool,
    /// The row where the prompt line lives (saved at start).
    prompt_row: u16,
}

impl InteractiveMode {
    pub fn new(input: Array, json_output: bool) -> Self {
        let prompt_row = cursor::position().map(|(_, row)| row).unwrap_or(0);
        Self {
            input,
            programme: String::new(),
            cursor: 0,
            json_output,
            show_help: false,
            prompt_row,
        }
    }

    /// Run interactive mode. Returns (programme, json_mode) if committed, None if cancelled.
    pub fn run(&mut self) -> Result<Option<(String, bool)>> {
        terminal::enable_raw_mode().context("failed to enable raw mode")?;
        let result = self.event_loop();
        terminal::disable_raw_mode().context("failed to disable raw mode")?;
        result
    }

    fn event_loop(&mut self) -> Result<Option<(String, bool)>> {
        let mut stdout = io::stdout();

        self.draw(&mut stdout)?;

        loop {
            match event::read().context("failed to read event")? {
                Event::Key(key) => {
                    match self.handle_key(key) {
                        KeyAction::Continue => {}
                        KeyAction::Commit => {
                            self.clear_output(&mut stdout)?;
                            return Ok(Some((self.programme.clone(), self.json_output)));
                        }
                        KeyAction::Cancel => {
                            self.clear_output(&mut stdout)?;
                            return Ok(None);
                        }
                    }
                    self.draw(&mut stdout)?;
                }
                Event::Resize(_, height) => {
                    // Clamp prompt_row to be within the new terminal height
                    if self.prompt_row >= height {
                        self.prompt_row = height.saturating_sub(1);
                    }
                    self.draw(&mut stdout)?;
                }
                _ => {}
            }
        }
    }

    fn clear_output(&self, stdout: &mut io::Stdout) -> Result<()> {
        execute!(
            stdout,
            cursor::MoveToColumn(0),
            terminal::Clear(ClearType::FromCursorDown)
        )?;
        stdout.flush()?;
        Ok(())
    }

    fn terminal_width() -> usize {
        terminal::size().map(|(w, _)| w as usize).unwrap_or(80)
    }

    fn available_preview_lines(&self) -> usize {
        let (_, term_height) = terminal::size().unwrap_or((80, 24));
        // Lines available below prompt (subtract 1 for the prompt line itself)
        let lines_below = (term_height as usize).saturating_sub(self.prompt_row as usize + 1);
        lines_below.max(MIN_PREVIEW_LINES)
    }

    fn truncate_line(line: &str, max_width: usize) -> String {
        if line.len() <= max_width {
            line.to_string()
        } else if max_width > 3 {
            format!("{}...", &line[..max_width - 3])
        } else {
            line[..max_width].to_string()
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> KeyAction {
        // Esc dismisses help, other keys pass through
        if self.show_help {
            if matches!(key.code, KeyCode::Esc) {
                self.show_help = false;
                return KeyAction::Continue;
            }
            self.show_help = false;
        }

        match (key.code, key.modifiers) {
            // Ctrl+C or Escape: cancel
            (KeyCode::Char('c'), KeyModifiers::CONTROL) | (KeyCode::Esc, _) => KeyAction::Cancel,

            // Ctrl+D: cancel if line is empty
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                if self.programme.is_empty() {
                    KeyAction::Cancel
                } else {
                    KeyAction::Continue
                }
            }

            // Ctrl+J: toggle JSON output
            (KeyCode::Char('j'), KeyModifiers::CONTROL) => {
                self.json_output = !self.json_output;
                KeyAction::Continue
            }

            // Ctrl+H: show help
            (KeyCode::Char('h'), KeyModifiers::CONTROL) => {
                self.show_help = true;
                KeyAction::Continue
            }

            // Enter: commit
            (KeyCode::Enter, _) => KeyAction::Commit,

            // Backspace: delete char before cursor
            (KeyCode::Backspace, _) => {
                if self.cursor > 0 {
                    self.programme.remove(self.cursor - 1);
                    self.cursor -= 1;
                }
                KeyAction::Continue
            }

            // Delete: delete char at cursor
            (KeyCode::Delete, _) => {
                if self.cursor < self.programme.len() {
                    self.programme.remove(self.cursor);
                }
                KeyAction::Continue
            }

            // Left arrow: move cursor left
            (KeyCode::Left, _) => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                KeyAction::Continue
            }

            // Right arrow: move cursor right
            (KeyCode::Right, _) => {
                if self.cursor < self.programme.len() {
                    self.cursor += 1;
                }
                KeyAction::Continue
            }

            // Home: move cursor to start
            (KeyCode::Home, _) => {
                self.cursor = 0;
                KeyAction::Continue
            }

            // End: move cursor to end
            (KeyCode::End, _) => {
                self.cursor = self.programme.len();
                KeyAction::Continue
            }

            // Regular character: insert at cursor
            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                self.programme.insert(self.cursor, c);
                self.cursor += 1;
                KeyAction::Continue
            }

            _ => KeyAction::Continue,
        }
    }

    fn draw(&mut self, stdout: &mut io::Stdout) -> Result<()> {
        let term_width = Self::terminal_width();
        let max_lines = self.available_preview_lines();

        // Pre-compute output content before clearing screen to reduce flicker
        let output_content = if self.show_help {
            None
        } else {
            Some(self.try_execute(max_lines))
        };

        // Move to saved prompt row and clear from there down
        execute!(
            stdout,
            cursor::MoveTo(0, self.prompt_row),
            terminal::Clear(ClearType::FromCursorDown)
        )?;

        // Draw prompt
        let prompt = format!("t> {}", self.programme);
        let help_hint = "^H Help";
        let help_col = term_width.saturating_sub(help_hint.len()) as u16;
        execute!(
            stdout,
            Print(&prompt),
            cursor::MoveToColumn(help_col),
            SetForegroundColor(Color::DarkGrey),
            Print(help_hint),
            ResetColor
        )?;

        // Count lines below prompt
        let mut lines_below = 0;

        if self.show_help {
            for help_line in OPERATOR_HELP.iter().take(max_lines) {
                execute!(stdout, Print("\r\n"))?;
                match help_line {
                    HelpLine::Heading(text) => {
                        execute!(
                            stdout,
                            SetForegroundColor(Color::Yellow),
                            Print(text),
                            ResetColor
                        )?;
                    }
                    HelpLine::Row(op1, desc1, op2, desc2) => {
                        execute!(
                            stdout,
                            Print("  "),
                            SetForegroundColor(Color::Cyan),
                            Print(format!("{:<OP_WIDTH$}", op1)),
                            ResetColor,
                            Print(format!("{:<DESC_WIDTH$}", desc1)),
                            SetForegroundColor(Color::Cyan),
                            Print(format!("{:<OP_WIDTH$}", op2)),
                            ResetColor,
                            Print(*desc2)
                        )?;
                    }
                    HelpLine::Single(op, desc) => {
                        execute!(
                            stdout,
                            Print("  "),
                            SetForegroundColor(Color::Cyan),
                            Print(format!("{:<OP_WIDTH$}", op)),
                            ResetColor,
                            Print(*desc)
                        )?;
                    }
                }
                lines_below += 1;
            }
            // Keys line
            if lines_below < max_lines {
                execute!(
                    stdout,
                    Print("\r\n"),
                    SetForegroundColor(Color::Yellow),
                    Print("Keys:"),
                    ResetColor
                )?;
                lines_below += 1;
            }
            if lines_below < max_lines {
                execute!(stdout, Print("\r\n  "))?;
                for (i, (key, desc)) in INTERACTIVE_KEYS.iter().enumerate() {
                    if i > 0 {
                        execute!(stdout, Print("  "))?;
                    }
                    execute!(
                        stdout,
                        SetForegroundColor(Color::Cyan),
                        Print(*key),
                        ResetColor,
                        Print(" "),
                        Print(*desc)
                    )?;
                }
                lines_below += 1;
            }
        } else {
            // Use pre-computed output
            let (value, depth, error) = output_content.unwrap();
            let error_info = error.as_ref().map(parse_error_info);
            let display_lines = if error_info.is_some() {
                max_lines.saturating_sub(1)
            } else {
                max_lines
            };

            // Show error first if present
            if let Some((offset, message)) = error_info {
                let caret_pos = 3 + offset; // "t> " is 3 chars
                let caret_line = format!("{:>width$}", "^", width = caret_pos + 1);
                let error_line = format!("{} {}", caret_line, message);
                let truncated = Self::truncate_line(&error_line, term_width);
                execute!(
                    stdout,
                    Print("\r\n"),
                    SetForegroundColor(Color::Red),
                    Print(&truncated),
                    ResetColor
                )?;
                lines_below += 1;
            }

            // Show output
            if self.json_output {
                execute!(stdout, Print("\r\n"))?;
                write_json_preview(stdout, &value, depth, term_width, display_lines)?;
                lines_below += count_json_preview_lines(&value, display_lines);
            } else {
                for (i, line) in format_text_with_depth(&value, depth)
                    .iter()
                    .take(display_lines)
                    .enumerate()
                {
                    let truncated = Self::truncate_line(line, term_width);
                    execute!(stdout, Print("\r\n"))?;
                    // Highlight first line at depth 0
                    if depth == 0 && i == 0 {
                        execute!(
                            stdout,
                            SetAttribute(Attribute::Bold),
                            Print(&truncated),
                            SetAttribute(Attribute::Reset)
                        )?;
                    } else {
                        execute!(stdout, Print(&truncated))?;
                    }
                    lines_below += 1;
                }
            }
        }

        // After printing output, check if the terminal scrolled.
        // If we printed lines_below lines starting from prompt_row, we expect
        // the cursor to be at prompt_row + lines_below. If scrolling occurred,
        // the cursor will be at a lower row (closer to bottom) than expected
        // relative to prompt_row, meaning prompt_row needs to be adjusted.
        let (_, current_row) = cursor::position().unwrap_or((0, 0));
        let expected_row = self.prompt_row + lines_below as u16;
        if current_row < expected_row {
            // Terminal scrolled - adjust prompt_row by the scroll amount
            let scroll_amount = expected_row - current_row;
            self.prompt_row = self.prompt_row.saturating_sub(scroll_amount);
        }

        // Move cursor back to prompt line at the right position
        if lines_below > 0 {
            execute!(stdout, cursor::MoveUp(lines_below as u16))?;
        }
        let cursor_col = 3 + self.cursor; // "t> " is 3 chars
        execute!(stdout, cursor::MoveToColumn(cursor_col as u16))?;

        stdout.flush()?;
        Ok(())
    }

    /// Try to execute the programme. Returns (value, depth, optional error).
    fn try_execute(&self, needed_lines: usize) -> (Value, usize, Option<anyhow::Error>) {
        // Try parsing the full programme
        let parse_result = parser::parse_programme(&self.programme);

        let (programme, parse_error) = match parse_result {
            Ok(prog) => (prog, None),
            Err(e) => {
                // Try to find the longest valid prefix
                let mut valid_prog = None;
                for i in (0..self.programme.len()).rev() {
                    if let Ok(prog) = parser::parse_programme(&self.programme[..i])
                        && !prog.operators.is_empty()
                    {
                        valid_prog = Some(prog);
                        break;
                    }
                }
                (
                    valid_prog.unwrap_or(crate::ast::Programme { operators: vec![] }),
                    Some(anyhow::anyhow!("{}", e)),
                )
            }
        };

        let depth = compute_depth(&programme);

        // Compile and run whatever we successfully parsed
        let ops = match interpreter::compile(&programme) {
            Ok(ops) => ops,
            Err(e) => return (Value::Array(self.input.deep_copy()), depth, Some(e.into())),
        };

        // Check if any operator requires full input (sort, dedupe, count, etc.)
        let requires_full_input = ops.iter().any(|op| op.requires_full_input());

        // Use adaptive batching if safe, otherwise process all input
        let batch_sizes: &[usize] = if requires_full_input {
            &[usize::MAX]
        } else {
            PREVIEW_BATCH_SIZES
        };

        for &batch_size in batch_sizes {
            let input = if batch_size >= self.input.len() {
                self.input.deep_copy()
            } else {
                self.input.truncated_copy(batch_size)
            };

            let mut ctx = interpreter::Context::new(Value::Array(input));

            if let Err(e) = interpreter::run(&ops, &mut ctx) {
                return (ctx.into_value(), depth, Some(e.into()));
            }

            let result = ctx.into_value();
            let output_lines = count_output_lines(&result);

            // If we have enough lines or processed all input, return
            if output_lines >= needed_lines || batch_size >= self.input.len() {
                return (result, depth, parse_error);
            }
        }

        unreachable!()
    }

    /// Get the full input for final execution after commit.
    pub fn full_input(&self) -> Array {
        self.input.deep_copy()
    }
}

enum KeyAction {
    Continue,
    Commit,
    Cancel,
}

/// Count the number of output lines a value would produce when displayed.
fn count_output_lines(value: &Value) -> usize {
    match value {
        Value::Array(arr) => arr.len(),
        Value::Text(s) => s.lines().count().max(1),
        Value::Number(_) => 1,
    }
}

/// Format a value as text with depth highlighting marker.
/// At depth 0, the first line is the "current unit".
/// At depth 1+, the first element within each line is highlighted.
fn format_text_with_depth(value: &Value, depth: usize) -> Vec<String> {
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

/// Count how many lines the JSON preview will use.
fn count_json_preview_lines(value: &Value, max_lines: usize) -> usize {
    match value {
        Value::Array(arr) => (arr.len() + 2).min(max_lines), // +2 for [ and ]
        _ => 1,
    }
}

/// Write JSON preview with depth-based highlighting.
fn write_json_preview<W: io::Write>(
    w: &mut W,
    value: &Value,
    depth: usize,
    max_width: usize,
    max_lines: usize,
) -> io::Result<()> {
    match value {
        Value::Array(arr) => {
            write_json_punct(w, "[")?;
            let mut lines_written = 1;
            for (i, elem) in arr.elements.iter().enumerate() {
                if lines_written >= max_lines {
                    break;
                }
                write!(w, "\r\n  ")?;
                // At depth 0, highlight entire first element
                // At depth > 0, pass depth down to highlight nested element
                if i == 0 {
                    write_json_value(w, elem, depth == 0, depth, max_width - 2)?;
                } else {
                    write_json_value(w, elem, false, 0, max_width - 2)?;
                }
                if i < arr.elements.len() - 1 {
                    write_json_punct(w, ",")?;
                }
                lines_written += 1;
            }
            if lines_written < max_lines {
                write!(w, "\r\n")?;
                write_json_punct(w, "]")?;
            }
        }
        _ => {
            let highlight = depth == 0;
            write_json_value(w, value, highlight, 0, max_width)?;
        }
    }
    Ok(())
}

/// Write a JSON value with syntax highlighting. If `highlight` is true, the entire value is bolded.
/// `depth` indicates how many levels to descend to find the element to highlight (1 = first child).
fn write_json_value<W: io::Write>(
    w: &mut W,
    value: &Value,
    highlight: bool,
    depth: usize,
    _max_width: usize,
) -> io::Result<()> {
    if highlight {
        write!(w, "{}", SetAttribute(Attribute::Bold))?;
        write_json_compact_highlighted(w, value)?;
        write!(w, "{}", SetAttribute(Attribute::NoBold))?;
    } else if depth > 0 {
        // Need to descend into the structure to highlight a nested element
        match value {
            Value::Array(arr) if !arr.elements.is_empty() => {
                write_json_punct(w, "[")?;
                for (i, elem) in arr.elements.iter().enumerate() {
                    if i > 0 {
                        write_json_punct(w, ",")?;
                    }
                    if i == 0 {
                        // First element: highlight if depth==1, otherwise recurse deeper
                        write_json_value(w, elem, depth == 1, depth - 1, _max_width)?;
                    } else {
                        write_json_compact_highlighted(w, elem)?;
                    }
                }
                write_json_punct(w, "]")?;
            }
            _ => write_json_compact_highlighted(w, value)?,
        }
    } else {
        write_json_compact_highlighted(w, value)?;
    }
    Ok(())
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

/// Compute the current depth from a parsed programme.
/// Depth increases with `@` (descend) and decreases with `^` (ascend).
fn compute_depth(programme: &ast::Programme) -> usize {
    let mut depth: isize = 0;
    for op in &programme.operators {
        match op {
            ast::Operator::Descend => depth += 1,
            ast::Operator::Ascend => depth = (depth - 1).max(0),
            _ => {}
        }
    }
    depth.max(0) as usize
}

/// Extract error offset and message from a parse error string.
fn parse_error_info(err: &anyhow::Error) -> (usize, String) {
    let err_str = err.to_string();

    // Parse errors from our parser look like:
    // "parse error: expected <selection>\n  sg\n    ^"
    // The input line has a 2-space prefix, so we subtract 2 from caret position

    if err_str.rfind('^').is_some() {
        let lines: Vec<&str> = err_str.lines().collect();
        if lines.len() >= 3 {
            let caret_line = lines[lines.len() - 1];
            let caret_pos = caret_line.find('^').unwrap_or(0);
            // Subtract 2 for the "  " prefix in the error format
            let offset = caret_pos.saturating_sub(2);
            let message = lines[0]
                .strip_prefix("parse error: ")
                .unwrap_or(lines[0])
                .to_string();
            return (offset, message);
        }
    }

    // Fallback for runtime errors or unexpected format
    (0, err_str)
}
