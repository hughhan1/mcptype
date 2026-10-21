use clap::{Parser, Subcommand};
use mcptype::{Expectation, Metadata, Request, changes};
use std::{error::Error, fs, io, path::PathBuf, process};

#[derive(Parser)]
#[command(version, about = "Compile and compare MCP contracts")]
struct Cli {
    #[command(subcommand)]
    command: Operation,
}

#[derive(Subcommand)]
enum Operation {
    /// Compile a schema catalog read from stdin.
    Compile,
    /// Compare a caller expectation with published metadata.
    Diff {
        expected: PathBuf,
        published: PathBuf,
    },
    /// Print a published JSON Schema.
    Schema {
        #[command(subcommand)]
        schema: SchemaCommand,
    },
}

#[derive(Subcommand)]
enum SchemaCommand {
    Contract,
}

fn run() -> Result<i32, Box<dyn Error>> {
    match Cli::parse().command {
        Operation::Compile => {
            let request: Request = serde_json::from_reader(io::stdin().lock())?;
            println!("{}", serde_json::to_string(&mcptype::compile(request)?)?);
            Ok(0)
        }
        Operation::Diff {
            expected,
            published,
        } => {
            let expected: Expectation = serde_json::from_slice(&fs::read(expected)?)?;
            let actual: Option<Metadata> = serde_json::from_slice(&fs::read(published)?)?;
            expected.validate().map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid expectation: {error}"),
                )
            })?;
            if let Some(actual) = &actual {
                actual.validate().map_err(|error| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("invalid metadata: {error}"),
                    )
                })?;
            }
            let differences = changes(&expected, actual.as_ref());
            for change in &differences {
                println!("{change}");
            }
            Ok(if differences.is_empty() { 0 } else { 1 })
        }
        Operation::Schema {
            schema: SchemaCommand::Contract,
        } => {
            print!("{}", mcptype::contract::CONTRACT_SCHEMA);
            Ok(0)
        }
    }
}

fn main() {
    match run() {
        Ok(0) => {}
        Ok(status) => process::exit(status),
        Err(error) => {
            eprintln!("mcptype: {error}");
            process::exit(2);
        }
    }
}
