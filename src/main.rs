use std::io::{self, Write};
use std::path::PathBuf;

use clap::Parser;

mod ast;
mod error;
mod interactive;
mod interpreter;
mod operators;
mod parser;
mod value;

use interpreter::Context;
use value::{Array, Level, Value};

fn about_text() -> String {
    format!(
        r#"T is a concise language for manipulating text, replacing common usage
patterns of Unix utilities like grep, sed, cut, awk, sort, and uniq.

Example:
Top 20 most frequent words (lowercased):

  t 'sjldo:20' file
    s   - split lines into words
    j   - flatten into single list
    l   - lowercase each word
    d   - dedupe with counts
    o   - sort descending
    :20 - take first 20

{}

For full documentation, see: https://github.com/alecthomas/t"#,
        interactive::help_text()
    )
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
}

fn main() {
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
        run_interactive(array);
    } else {
        run_batch(&prog, array, cli.json);
    }
}

fn run_interactive(input: Array) {
    let mut mode = interactive::InteractiveMode::new(input);
    match mode.run() {
        Ok(Some((prog, json))) => {
            // User committed - run full programme on full input
            let input = mode.full_input();
            run_batch(&prog, input, json);
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

fn run_batch(prog: &str, array: Array, json: bool) {
    let programme = match parser::parse_programme(prog) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    let ops = match interpreter::compile(&programme) {
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
    let mut handle = stdout.lock();
    if json {
        interactive::write_json_highlighted(&mut handle, &value).expect("JSON output failed");
    } else {
        write!(handle, "{}", value).expect("write failed");
    }
    writeln!(handle).expect("write failed");
}
