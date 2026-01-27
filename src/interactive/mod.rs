//! Interactive mode for live previewing programmes.

mod help;
mod json;
mod text;

use std::io::{self, Write};
use std::time::Instant;

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

pub use help::help_text;
pub use json::write_json_highlighted;

/// Batch sizes for adaptive preview execution.
const PREVIEW_BATCH_SIZES: &[usize] = &[100, 500, 2000, usize::MAX];

pub struct InteractiveMode {
    input: Array,
    programme: String,
    cursor: usize,
    json_output: bool,
    show_help: bool,
    /// The row where the prompt line lives (saved at start).
    prompt_row: u16,
    /// Cached formatted output: (programme, json_output, needed_lines) -> formatted lines
    cached_output: Option<CachedOutput>,
}

struct CachedOutput {
    programme: String,
    json_output: bool,
    needed_lines: usize,
    /// Formatted output lines ready for display
    lines: Vec<String>,
    /// Depth for highlighting
    depth: usize,
    /// Error info if any: (offset, message)
    error_info: Option<(usize, String)>,
}

impl InteractiveMode {
    pub fn new(input: Array, json_output: bool) -> Self {
        Self {
            input,
            programme: String::new(),
            cursor: 0,
            json_output,
            show_help: false,
            prompt_row: 0,
            cached_output: None,
        }
    }

    /// Run interactive mode. Returns (programme, json_mode) if committed, None if cancelled.
    pub fn run(&mut self) -> Result<Option<(String, bool)>> {
        terminal::enable_raw_mode().context("failed to enable raw mode")?;
        // Query cursor position after enabling raw mode - some terminals require
        // raw mode for the position query to work correctly
        self.prompt_row = cursor::position().map(|(_, row)| row).unwrap_or(0);
        let result = self.event_loop();
        terminal::disable_raw_mode().context("failed to disable raw mode")?;
        result
    }

    fn event_loop(&mut self) -> Result<Option<(String, bool)>> {
        let mut stdout = io::stdout();

        self.draw(&mut stdout, None)?;

        loop {
            match event::read().context("failed to read event")? {
                Event::Key(key) => {
                    let start = Instant::now();
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
                    self.draw(&mut stdout, Some(start))?;
                }
                Event::Resize(_, height) => {
                    // Clamp prompt_row to be within the new terminal height
                    if self.prompt_row >= height {
                        self.prompt_row = height.saturating_sub(1);
                    }
                    self.draw(&mut stdout, None)?;
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
        // Use help line count as minimum so help is never truncated
        lines_below.max(help::help_line_count())
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

    fn draw(&mut self, stdout: &mut io::Stdout, start: Option<Instant>) -> Result<()> {
        // Use term_width - 1 to prevent auto-wrap when line fills last column
        let term_width = Self::terminal_width().saturating_sub(1).max(1);
        let max_lines = self.available_preview_lines();

        // Get cached or compute formatted output before clearing screen to reduce flicker
        let output = if self.show_help {
            None
        } else {
            Some(self.get_formatted_output(max_lines, term_width))
        };

        // Move to saved prompt row and clear from there down
        execute!(
            stdout,
            cursor::MoveTo(0, self.prompt_row),
            terminal::Clear(ClearType::FromCursorDown)
        )?;

        // Draw prompt with help hint on the right (timing added at end)
        let prompt = format!("t> {}", self.programme);
        let help_hint = "^H Help";
        execute!(stdout, Print(&prompt),)?;

        // Count lines below prompt
        let mut lines_below = 0;

        if self.show_help {
            lines_below = help::draw_help(stdout, max_lines)?;
        } else {
            let (lines, depth, error_info) = output.unwrap();

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

            // Show pre-formatted output lines (limit to max_lines in case cache has more)
            for (i, line) in lines.iter().take(max_lines).enumerate() {
                execute!(stdout, Print("\r\n"))?;
                // Highlight first line at depth 0 (only for non-JSON output)
                if !self.json_output && depth == 0 && i == 0 {
                    execute!(
                        stdout,
                        SetAttribute(Attribute::Bold),
                        Print(line),
                        SetAttribute(Attribute::NormalIntensity)
                    )?;
                } else {
                    execute!(stdout, Print(line))?;
                }
                lines_below += 1;
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

        // Draw timing and help hint on the right side of prompt line
        let timing = start.map(|s| format!("{:.1}ms", s.elapsed().as_secs_f64() * 1000.0));
        let right_text = match &timing {
            Some(t) => format!("{} {}", t, help_hint),
            None => help_hint.to_string(),
        };
        let right_col = term_width.saturating_sub(right_text.len()) as u16;
        execute!(
            stdout,
            cursor::MoveToColumn(right_col),
            SetAttribute(Attribute::Dim),
            Print(&right_text),
            SetAttribute(Attribute::NormalIntensity)
        )?;

        let cursor_col = 3 + self.cursor; // "t> " is 3 chars
        execute!(stdout, cursor::MoveToColumn(cursor_col as u16))?;

        stdout.flush()?;
        Ok(())
    }

    /// Get formatted output lines, using cache if programme hasn't changed.
    /// Returns (lines, depth, error_info).
    fn get_formatted_output(
        &mut self,
        max_lines: usize,
        term_width: usize,
    ) -> (Vec<String>, usize, Option<(usize, String)>) {
        // Check if we can use cached result
        if let Some(ref cached) = self.cached_output
            && cached.programme == self.programme
            && cached.json_output == self.json_output
            && cached.needed_lines >= max_lines
        {
            return (
                cached.lines.clone(),
                cached.depth,
                cached.error_info.clone(),
            );
        }

        // Compute fresh result
        let (value, depth, error) = self.try_execute(max_lines);
        let error_info = error.as_ref().map(parse_error_info);

        let display_lines = if error_info.is_some() {
            max_lines.saturating_sub(1)
        } else {
            max_lines
        };

        // Format output lines
        let lines: Vec<String> = if self.json_output {
            json::format_json_preview(&value, depth, display_lines, term_width)
        } else {
            text::format_text_with_depth(&value, depth, display_lines, term_width)
        };

        // Cache the result
        self.cached_output = Some(CachedOutput {
            programme: self.programme.clone(),
            json_output: self.json_output,
            needed_lines: max_lines,
            lines: lines.clone(),
            depth,
            error_info: error_info.clone(),
        });

        (lines, depth, error_info)
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
            let output_lines = text::count_output_lines(&result);

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
