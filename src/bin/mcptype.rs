use clap::Parser;
use mcptype::{Expectation, Metadata, Request, changes};
use std::{error::Error, fs, io, path::PathBuf, process};

#[derive(Parser)]
#[command(version, about = "Compile and compare MCP contracts")]
enum Cli {
    /// Compile a request read from stdin.
    Compile,
    /// Print what breaks a caller's expectation if the published metadata is deployed; exit 1 if anything does.
    Diff {
        expected: PathBuf,
        published: PathBuf,
    },
}

fn run() -> Result<bool, Box<dyn Error>> {
    match Cli::parse() {
        Cli::Compile => {
            let request: Request = serde_json::from_reader(io::stdin().lock())?;
            println!("{}", serde_json::to_string(&mcptype::compile(request)?)?);
            Ok(true)
        }
        Cli::Diff {
            expected,
            published,
        } => {
            let expected: Expectation = serde_json::from_slice(&fs::read(expected)?)?;
            let published: Option<Metadata> = serde_json::from_slice(&fs::read(published)?)?;
            let changes = changes(&expected, published.as_ref());
            for change in &changes {
                println!("{change}");
            }
            Ok(changes.is_empty())
        }
    }
}

fn main() {
    match run() {
        Ok(true) => {}
        Ok(false) => process::exit(1),
        Err(error) => {
            eprintln!("mcptype: {error}");
            process::exit(2);
        }
    }
}
