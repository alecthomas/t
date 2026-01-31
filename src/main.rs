use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

use clap::{CommandFactory, Parser};

mod ast;
mod error;
mod interactive;
mod interpreter;
mod operators;
mod parser;
mod value;

use interpreter::{CompileConfig, Context};
use operators::{JoinMode, SplitMode};
use value::{Array, Level, Value};

const ABOUT_INTRO: &str = r#"T is a concise language for manipulating text, replacing common usage
patterns of Unix utilities like grep, sed, cut, awk, sort, and uniq.

Example:
Top 20 most frequent words (lowercased):

  t 'sfldo:20' file

    s   - split lines into words
    f   - flatten into single list
    l   - lowercase each word
    d   - dedupe with counts
    o   - sort descending
    :20 - take first 20

"#;

const ABOUT_FOOTER: &str = "For full documentation, see: https://github.com/alecthomas/t";

fn about_text() -> String {
    format!(
        "{}\n{}\n\n{}",
        ABOUT_INTRO,
        interactive::help_text(),
        ABOUT_FOOTER
    )
}

fn print_help(use_color: bool, cmd: &clap::Command) -> io::Result<()> {
    let mut stdout = io::stdout();
    interactive::write_intro(&mut stdout, use_color)?;
    interactive::write_help_text(&mut stdout, use_color)?;
    writeln!(stdout)?;
    interactive::write_footer(&mut stdout, use_color)?;
    writeln!(stdout)?;
    interactive::write_options(&mut stdout, cmd, use_color)?;
    Ok(())
}

#[derive(Parser)]
#[command(name = "t")]
#[command(about = about_text())]
struct Cli {
    /// Programme to execute
    #[arg(default_value = "")]
    prog: String,

    /// Optional files to process
    files: Vec<String>,

    /// Output as JSON
    #[arg(short = 'j', long = "json")]
    json: bool,

    /// Interactive mode
    #[arg(short = 'i', long = "interactive")]
    interactive: bool,

    /// Print equivalent command line (with -i)
    #[arg(short = 'p', long = "print")]
    print_command: bool,

    /// Input delimiter (what `s` splits on)
    #[arg(short = 'd')]
    input_delim: Option<String>,

    /// Output delimiter (what `j` joins with)
    #[arg(short = 'D')]
    output_delim: Option<String>,

    /// CSV mode (split/join use CSV parsing)
    #[arg(short = 'c', long = "csv")]
    csv: bool,

    /// Debug mode (show semantic level before arrays)
    #[arg(long = "debug")]
    debug: bool,
}

fn main() {
    // Handle --help ourselves for colored output
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        let use_color = io::stdout().is_terminal();
        let mut cmd = Cli::command();
        cmd.build();
        let result = print_help(use_color, &cmd);
        if let Err(e) = result
            && e.kind() != io::ErrorKind::BrokenPipe
        {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
        return;
    }

    let cli = Cli::parse();

    // In interactive mode, prog is treated as the first file argument
    let (prog, files) = if cli.interactive {
        let mut all_files = Vec::new();
        if !cli.prog.is_empty() {
            all_files.push(cli.prog.clone());
        }
        all_files.extend(cli.files.iter().cloned());
        (String::new(), all_files)
    } else {
        (cli.prog.clone(), cli.files.clone())
    };

    if cli.interactive && files.is_empty() {
        eprintln!("Error: interactive mode requires file arguments (cannot read from stdin)");
        std::process::exit(1);
    }

    // Build compile config from CLI flags
    let config = build_compile_config(&cli);

    // Check which files are regular files (before reading, as pipes become invalid after)
    let regular_files: Vec<_> = files
        .iter()
        .filter(|f| {
            std::fs::metadata(f)
                .map(|m| m.file_type().is_file())
                .unwrap_or(false)
        })
        .cloned()
        .collect();

    let input = if files.is_empty() {
        Array::from_stdin(Level::Line)
    } else {
        let paths: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();
        Array::from_files(&paths, Level::Line)
    };

    let array = match input {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error reading input: {}", e);
            std::process::exit(1);
        }
    };

    if cli.interactive {
        run_interactive(
            array,
            &regular_files,
            cli.print_command,
            cli.json,
            cli.debug,
            &config,
        );
    } else {
        run_batch(&prog, array, cli.json, cli.debug, &config);
    }
}

fn build_compile_config(cli: &Cli) -> CompileConfig {
    let split_mode = if cli.csv {
        SplitMode::Csv
    } else if let Some(ref delim) = cli.input_delim {
        SplitMode::Delimiter(delim.clone())
    } else {
        SplitMode::Whitespace
    };

    let join_mode = if cli.csv {
        JoinMode::Csv
    } else if let Some(ref delim) = cli.output_delim {
        JoinMode::Delimiter(delim.clone())
    } else {
        JoinMode::Semantic
    };

    CompileConfig {
        split_mode,
        join_mode,
    }
}

fn run_interactive(
    input: Array,
    files: &[String],
    print_command: bool,
    json: bool,
    debug: bool,
    config: &CompileConfig,
) {
    let mut mode =
        interactive::InteractiveMode::new_with_config(input, json, debug, config.clone());
    match mode.run() {
        Ok(Some((prog, json, debug))) => {
            // User committed - run full programme on full input
            let input = mode.full_input();
            run_batch(&prog, input, json, debug, config);

            // Print equivalent command line
            if print_command {
                eprint!("t");
                if json {
                    eprint!(" -j");
                }
                eprint!(" '{}'", prog);
                for file in files {
                    if file.contains(char::is_whitespace) || file.contains('\'') {
                        eprint!(" '{}'", file.replace('\'', "'\\''"));
                    } else {
                        eprint!(" {}", file);
                    }
                }
                eprintln!();
            }
        }
        Ok(None) => {
            // User cancelled
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

fn run_batch(prog: &str, array: Array, json: bool, debug: bool, config: &CompileConfig) {
    let programme = match parser::parse_programme(prog) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    let ops = match interpreter::compile_with_config(&programme, config) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };
    let mut ctx = Context::new(Value::Array(array));

    if let Err(e) = interpreter::run(&ops, &mut ctx) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    let value = ctx.into_value();
    let stdout = io::stdout();
    let use_color = stdout.is_terminal();
    let mut handle = stdout.lock();
    let result = if debug {
        interactive::write_json_debug(&mut handle, &value, use_color)
            .and_then(|()| writeln!(handle))
    } else if json {
        interactive::write_json_highlighted(&mut handle, &value, use_color)
            .and_then(|()| writeln!(handle))
    } else {
        write!(handle, "{}", value).and_then(|()| writeln!(handle))
    };
    if let Err(e) = result
        && e.kind() != io::ErrorKind::BrokenPipe
    {
        eprintln!("write failed: {}", e);
        std::process::exit(1);
    }
}
