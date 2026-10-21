use base64::{Engine, engine::general_purpose::STANDARD};
use clap::{Parser, Subcommand};
use mcptype::{Expectation, GenerateRequest, Metadata, Request, changes};
use serde_json::Value;
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
    /// Write a generated package into a directory; the request's `runtime` values are file paths.
    Generate { request: PathBuf, output: PathBuf },
    /// Print the published contract JSON Schema.
    Schema {
        #[command(subcommand)]
        schema: SchemaCommand,
    },
}

#[derive(Subcommand)]
enum SchemaCommand {
    Contract,
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
        Cli::Generate { request, output } => {
            let mut request: Value = serde_json::from_slice(&fs::read(request)?)?;
            if let Some(Value::Object(runtime)) = request.get_mut("runtime") {
                for data in runtime.values_mut() {
                    let path = data.as_str().ok_or("runtime values must be file paths")?;
                    *data = STANDARD.encode(fs::read(path)?).into();
                }
            }
            let request: GenerateRequest = serde_json::from_value(request)?;
            for (path, data) in mcptype::generate(request)?.files {
                let path = output.join(path);
                fs::create_dir_all(path.parent().unwrap_or(&output))?;
                fs::write(path, data)?;
            }
            Ok(true)
        }
        Cli::Schema {
            schema: SchemaCommand::Contract,
        } => {
            print!("{}", include_str!("../../schema/contract.schema.json"));
            Ok(true)
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
