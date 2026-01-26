use std::io::{self, Write};
use std::path::PathBuf;

use clap::Parser;

mod ast;
mod error;
mod interpreter;
mod operators;
mod parser;
mod value;

use interpreter::Context;
use value::{Array, Level, Value};

#[derive(Parser)]
#[command(name = "t")]
#[command(
    about = r#"T is a concise language for manipulating text, replacing common usage
patterns of Unix utilities like grep, sed, cut, awk, sort, and uniq.

Example - Top 20 most frequent words (lowercased):
  t 'sjldo:20' file
    s   - split lines into words
    j   - flatten into single list
    l   - lowercase each word
    d   - dedupe with counts
    o   - sort descending
    :20 - take first 20

Operators:
  Split/Join:    s S<delim> j J<delim>
  Transform:     l L<sel> u U<sel> r/old/new/ R<sel>/old/new/ n N<sel> t T<sel>
  Filter:        /regex/ !/regex/ x
  Select/Group:  <selection> o O g<sel> d D # + c C<delim>
  Navigation:    @ ^

For full documentation, see: https://github.com/alecthomas/t
"#
)]
struct Cli {
    /// Programme to execute
    #[arg(default_value = "")]
    prog: String,

    /// Optional files to process
    files: Vec<String>,

    /// Output as JSON
    #[arg(short = 'j', long = "json")]
    json: bool,
}

fn main() {
    let cli = Cli::parse();

    let programme = match parser::parse_programme(&cli.prog) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    let input = if cli.files.is_empty() {
        Array::from_stdin(Level::Line)
    } else {
        let paths: Vec<PathBuf> = cli.files.iter().map(PathBuf::from).collect();
        Array::from_files(&paths, Level::Line)
    };

    let array = match input {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error reading input: {}", e);
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
    if cli.json {
        serde_json::to_writer_pretty(&mut handle, &value).expect("JSON serialization failed");
    } else {
        write!(handle, "{}", value).expect("write failed");
    }
    writeln!(handle).expect("write failed");
}
