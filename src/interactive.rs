//! Interactive mode for live previewing programmes.

use std::io::{self, Write};

use anyhow::{Context, Result};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, ClearType},
};

use crate::interpreter;
use crate::parser;
use crate::value::{Array, Value};

const MIN_PREVIEW_LINES: usize = 10;

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
    HelpLine::Row("x", "delete empty", "d", "dedupe with counts"),
    HelpLine::Row("D", "dedupe", "o", "sort descending"),
    HelpLine::Row("O", "sort ascending", "g<sel>", "group by"),
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
    last_output_lines: usize,
    prompt_row: u16,
}

impl InteractiveMode {
    pub fn new(input: Array) -> Self {
        Self {
            input,
            programme: String::new(),
            cursor: 0,
            json_output: false,
            show_help: false,
            last_output_lines: 0,
            prompt_row: 0,
        }
    }

    /// Run interactive mode. Returns (programme, json_mode) if committed, None if cancelled.
    pub fn run(&mut self) -> Result<Option<(String, bool)>> {
        // Capture cursor position before raw mode
        self.prompt_row = cursor::position().map(|(_, row)| row).unwrap_or(0);

        terminal::enable_raw_mode().context("failed to enable raw mode")?;
        let result = self.event_loop();
        terminal::disable_raw_mode().context("failed to disable raw mode")?;
        result
    }

    fn event_loop(&mut self) -> Result<Option<(String, bool)>> {
        let mut stdout = io::stdout();

        self.draw(&mut stdout)?;

        loop {
            if let Event::Key(key) = event::read().context("failed to read event")? {
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
        }
    }

    fn clear_output(&self, stdout: &mut io::Stdout) -> Result<()> {
        // Move down to clear lines below prompt
        if self.last_output_lines > 0 {
            execute!(stdout, cursor::MoveDown(self.last_output_lines as u16))?;
            for _ in 0..self.last_output_lines {
                execute!(
                    stdout,
                    terminal::Clear(ClearType::CurrentLine),
                    cursor::MoveUp(1)
                )?;
            }
        }
        // Clear prompt line and leave cursor at start
        execute!(
            stdout,
            cursor::MoveToColumn(0),
            terminal::Clear(ClearType::CurrentLine)
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

        // Clear previous output: move down to end, then clear upward
        if self.last_output_lines > 0 {
            execute!(stdout, cursor::MoveDown(self.last_output_lines as u16))?;
            for _ in 0..self.last_output_lines {
                execute!(
                    stdout,
                    terminal::Clear(ClearType::CurrentLine),
                    cursor::MoveUp(1)
                )?;
            }
        }

        // Draw prompt (cursor is now on prompt line)
        let prompt = format!("t> {}", self.programme);
        execute!(
            stdout,
            cursor::MoveToColumn(0),
            terminal::Clear(ClearType::CurrentLine),
            Print(&prompt)
        )?;

        // Count lines below prompt
        let mut lines_below = 0;
        let max_lines = self.available_preview_lines();

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
            // Try to parse and run
            match self.try_execute() {
                Ok(value) => {
                    if self.json_output {
                        let json = format_json_preview(&value);
                        for line in json.lines().take(max_lines) {
                            let truncated = Self::truncate_line(line, term_width);
                            execute!(stdout, Print("\r\n"))?;
                            write_highlighted_json_str(stdout, &truncated)?;
                            lines_below += 1;
                        }
                    } else {
                        let output = format!("{}", value);
                        for line in output.lines().take(max_lines) {
                            let truncated = Self::truncate_line(line, term_width);
                            execute!(stdout, Print("\r\n"), Print(&truncated))?;
                            lines_below += 1;
                        }
                    }
                }
                Err(err) => {
                    // Show error with caret
                    let (offset, message) = parse_error_info(&err);
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
            }
        }

        // Move cursor back to prompt line at the right position
        if lines_below > 0 {
            execute!(stdout, cursor::MoveUp(lines_below as u16))?;
        }
        let cursor_col = 3 + self.cursor; // "t> " is 3 chars
        execute!(stdout, cursor::MoveToColumn(cursor_col as u16))?;

        stdout.flush()?;
        self.last_output_lines = lines_below;

        Ok(())
    }

    fn try_execute(&self) -> Result<Value> {
        let programme =
            parser::parse_programme(&self.programme).map_err(|e| anyhow::anyhow!("{}", e))?;

        let ops = interpreter::compile(&programme)?;

        // Run on full input - display will be limited later
        let input = self.input.deep_copy();
        let mut ctx = interpreter::Context::new(Value::Array(input));
        interpreter::run(&ops, &mut ctx)?;

        Ok(ctx.into_value())
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

/// Format a value as JSON for preview - top-level arrays expand, inner arrays are compact.
fn format_json_preview(value: &Value) -> String {
    match value {
        Value::Array(arr) => {
            let mut lines = vec!["[".to_string()];
            for (i, elem) in arr.elements.iter().enumerate() {
                let comma = if i < arr.elements.len() - 1 { "," } else { "" };
                let compact = format_json_compact(elem);
                lines.push(format!("  {}{}", compact, comma));
            }
            lines.push("]".to_string());
            lines.join("\n")
        }
        _ => format_json_compact(value),
    }
}

/// Format a value as compact single-line JSON.
fn format_json_compact(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|e| format!("\"JSON error: {}\"", e))
}

/// Write syntax-highlighted JSON to a writer.
pub fn write_json_highlighted<W: io::Write>(w: &mut W, value: &Value) -> io::Result<()> {
    let json = serde_json::to_string_pretty(value).map_err(|e| io::Error::other(e.to_string()))?;
    write_highlighted_json_str(w, &json)
}

/// Write a JSON string with syntax highlighting.
fn write_highlighted_json_str<W: io::Write>(w: &mut W, json: &str) -> io::Result<()> {
    use crossterm::style::{Color, ResetColor, SetForegroundColor};

    let mut chars = json.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' => {
                // Collect the full string
                let mut s = String::from('"');
                while let Some(&ch) = chars.peek() {
                    s.push(chars.next().unwrap());
                    if ch == '"' {
                        break;
                    }
                    if ch == '\\' && chars.peek().is_some() {
                        s.push(chars.next().unwrap());
                    }
                }
                // Check if this is a key (followed by ':')
                let is_key = {
                    let mut peek_chars = chars.clone();
                    loop {
                        match peek_chars.next() {
                            Some(' ') | Some('\n') | Some('\r') | Some('\t') => continue,
                            Some(':') => break true,
                            _ => break false,
                        }
                    }
                };
                let color = if is_key { Color::Blue } else { Color::Green };
                write!(w, "{}{}{}", SetForegroundColor(color), s, ResetColor)?;
            }
            c if c.is_ascii_digit() || c == '-' => {
                // Number
                let mut num = String::from(c);
                while let Some(&ch) = chars.peek() {
                    if ch.is_ascii_digit()
                        || ch == '.'
                        || ch == 'e'
                        || ch == 'E'
                        || ch == '+'
                        || ch == '-'
                    {
                        num.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                write!(
                    w,
                    "{}{}{}",
                    SetForegroundColor(Color::Cyan),
                    num,
                    ResetColor
                )?;
            }
            't' | 'f' | 'n' => {
                // true, false, null
                let mut word = String::from(c);
                while let Some(&ch) = chars.peek() {
                    if ch.is_ascii_alphabetic() {
                        word.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                if word == "true" || word == "false" || word == "null" {
                    write!(
                        w,
                        "{}{}{}",
                        SetForegroundColor(Color::Yellow),
                        word,
                        ResetColor
                    )?;
                } else {
                    write!(w, "{}", word)?;
                }
            }
            _ => {
                write!(w, "{}", c)?;
            }
        }
    }
    Ok(())
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
