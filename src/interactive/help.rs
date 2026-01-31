//! Help text definitions and generation.

use clap::Command;
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
    HelpLine::Single("m/<pat>/", "matches to array"),
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

/// Write a single help line to a writer.
fn write_help_line<W: io::Write>(
    w: &mut W,
    help_line: &HelpLine,
    use_color: bool,
    newline: &str,
) -> io::Result<()> {
    match help_line {
        HelpLine::Heading(text) => {
            if use_color {
                execute!(
                    w,
                    SetForegroundColor(Color::Yellow),
                    Print(text),
                    ResetColor,
                    Print(newline)
                )?;
            } else {
                write!(w, "{}{}", text, newline)?;
            }
        }
        HelpLine::Row(op1, desc1, op2, desc2) => {
            if use_color {
                execute!(
                    w,
                    Print("  "),
                    SetForegroundColor(Color::Cyan),
                    Print(format!("{:<OP_WIDTH$}", op1)),
                    ResetColor,
                    Print(format!("{:<DESC_WIDTH$}", desc1)),
                    SetForegroundColor(Color::Cyan),
                    Print(format!("{:<OP_WIDTH$}", op2)),
                    ResetColor,
                    Print(*desc2),
                    Print(newline)
                )?;
            } else {
                write!(
                    w,
                    "  {:<OP_WIDTH$}{:<DESC_WIDTH$}{:<OP_WIDTH$}{}{}",
                    op1, desc1, op2, desc2, newline
                )?;
            }
        }
        HelpLine::Single(op, desc) => {
            if use_color {
                execute!(
                    w,
                    Print("  "),
                    SetForegroundColor(Color::Cyan),
                    Print(format!("{:<OP_WIDTH$}", op)),
                    ResetColor,
                    Print(*desc),
                    Print(newline)
                )?;
            } else {
                write!(w, "  {:<OP_WIDTH$}{}{}", op, desc, newline)?;
            }
        }
    }
    Ok(())
}

/// Write colored help text to a writer (for CLI --help).
pub fn write_help_text<W: io::Write>(w: &mut W, use_color: bool) -> io::Result<()> {
    for help_line in OPERATOR_HELP {
        write_help_line(w, help_line, use_color, "\n")?;
    }
    Ok(())
}

/// Write the intro section with optional coloring.
pub fn write_intro<W: io::Write>(w: &mut W, use_color: bool) -> io::Result<()> {
    let intro_before_example =
        "T is a concise language for manipulating text, replacing common usage
patterns of Unix utilities like grep, sed, cut, awk, sort, and uniq.

";
    let example_heading = "Example:";
    let intro_after_example = "
Top 20 most frequent words (lowercased):

  t 'sfldo:20' file
    s   - split lines into words
    f   - flatten into single list
    l   - lowercase each word
    d   - dedupe with counts
    o   - sort descending
    :20 - take first 20

";

    write!(w, "{}", intro_before_example)?;
    if use_color {
        execute!(
            w,
            SetForegroundColor(Color::Yellow),
            Print(example_heading),
            ResetColor
        )?;
    } else {
        write!(w, "{}", example_heading)?;
    }
    write!(w, "{}", intro_after_example)?;
    Ok(())
}

/// Write the footer section with optional coloring.
pub fn write_footer<W: io::Write>(w: &mut W, use_color: bool) -> io::Result<()> {
    let prefix = "For full documentation, see: ";
    let url = "https://github.com/alecthomas/t";

    write!(w, "{}", prefix)?;
    if use_color {
        execute!(
            w,
            SetForegroundColor(Color::Cyan),
            Print(url),
            ResetColor,
            Print("\n")
        )?;
    } else {
        writeln!(w, "{}", url)?;
    }
    Ok(())
}

/// Write the options section using clap's structured argument access.
pub fn write_options<W: io::Write>(w: &mut W, cmd: &Command, use_color: bool) -> io::Result<()> {
    // Write "Options:" heading
    if use_color {
        execute!(
            w,
            SetForegroundColor(Color::Yellow),
            Print("Options:"),
            ResetColor,
            Print("\n")
        )?;
    } else {
        writeln!(w, "Options:")?;
    }

    // Collect arguments and format them
    for arg in cmd.get_arguments() {
        // Skip hidden arguments and positional arguments
        if arg.is_hide_set() || arg.is_positional() {
            continue;
        }

        // Build the flags string
        let mut flags = String::new();
        if let Some(short) = arg.get_short() {
            flags.push('-');
            flags.push(short);
        }
        if let Some(long) = arg.get_long() {
            if !flags.is_empty() {
                flags.push_str(", ");
            }
            flags.push_str("--");
            flags.push_str(long);
        }

        // Add value name if it takes a value
        if arg.get_num_args().is_some_and(|n| n.takes_values())
            && let Some(value_names) = arg.get_value_names()
        {
            for name in value_names {
                flags.push(' ');
                flags.push('<');
                flags.push_str(name);
                flags.push('>');
            }
        }

        // Get the help text
        let help = arg.get_help().map(|s| s.to_string()).unwrap_or_default();

        // Calculate padding - flags column is roughly 19 chars wide
        let flags_width = 19;
        let padded_flags = format!("{:<width$}", flags, width = flags_width);

        if use_color {
            execute!(
                w,
                Print("  "),
                SetForegroundColor(Color::Cyan),
                Print(&padded_flags),
                ResetColor,
                Print(&help),
                Print("\n")
            )?;
        } else {
            writeln!(w, "  {}{}", padded_flags, help)?;
        }
    }

    Ok(())
}

/// Draw help content to stdout (for interactive mode).
pub fn draw_help(stdout: &mut io::Stdout, max_lines: usize) -> io::Result<usize> {
    let mut lines_below = 0;

    for help_line in OPERATOR_HELP.iter().take(max_lines) {
        execute!(stdout, Print("\r\n"))?;
        write_help_line(stdout, help_line, true, "")?;
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
