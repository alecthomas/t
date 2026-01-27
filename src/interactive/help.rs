//! Help text definitions and generation.

use crossterm::{
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
};
use std::io;

#[allow(dead_code)]
pub enum HelpLine {
    Heading(&'static str),
    Row(&'static str, &'static str, &'static str, &'static str),
    Single(&'static str, &'static str),
}

pub const OPERATOR_HELP: &[HelpLine] = &[
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
    HelpLine::Row(
        ";",
        "separator (no-op)",
        "<sel>",
        "select (e.g. 0, 1:3, ::2)",
    ),
];

pub const INTERACTIVE_KEYS: &[(&str, &str)] = &[
    ("Enter", "Commit"),
    ("^C/Esc", "Cancel"),
    ("^J", "JSON"),
    ("^H", "Help"),
];

const OP_WIDTH: usize = 16;
const DESC_WIDTH: usize = 21;

/// Returns the total number of lines in the help output.
pub fn help_line_count() -> usize {
    // OPERATOR_HELP lines + "Keys:" heading + keys row
    OPERATOR_HELP.len() + 2
}

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

/// Draw help content to stdout.
pub fn draw_help(stdout: &mut io::Stdout, max_lines: usize) -> io::Result<usize> {
    let mut lines_below = 0;

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

    Ok(lines_below)
}
