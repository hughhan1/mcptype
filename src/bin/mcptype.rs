use base64::{Engine, engine::general_purpose::STANDARD};
use clap::{Parser, Subcommand};
use mcptype::{Expectation, Metadata, Request, changes};
use serde_json::{Map, Value};
use std::{
    collections::BTreeMap,
    error::Error,
    fs, io,
    path::{Path, PathBuf},
    process,
};

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
    /// Generate a package from a JSON request whose runtime values name files on disk.
    Generate { request: PathBuf, output: PathBuf },
    /// Verify contract source fingerprints and run the local language type checker.
    Check {
        request: PathBuf,
        #[arg(long)]
        verify_only: bool,
    },
    /// Print a published JSON Schema.
    Schema {
        #[command(subcommand)]
        schema: SchemaCommand,
    },
}

#[derive(serde::Deserialize)]
struct FileGenerateRequest {
    target: mcptype::Target,
    identity: mcptype::Identity,
    #[serde(default)]
    requires: BTreeMap<String, mcptype::Identity>,
    #[serde(default)]
    contracts: BTreeMap<String, mcptype::Contract>,
    #[serde(default)]
    runtime: BTreeMap<String, PathBuf>,
    #[serde(default)]
    package_fields: Map<String, Value>,
    go_module: Option<String>,
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
        Operation::Generate { request, output } => {
            let request_path = request.clone();
            let bytes = fs::read(&request_path)?;
            let mut request: FileGenerateRequest = serde_json::from_slice(&bytes)?;
            let root = request_path_parent(&request_path);
            let runtime = std::mem::take(&mut request.runtime)
                .into_iter()
                .map(|(name, path)| {
                    let path = if path.is_absolute() {
                        path
                    } else {
                        root.join(path)
                    };
                    fs::read(path).map(|contents| (name, STANDARD.encode(contents)))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()?;
            let input = serde_json::to_vec(&serde_json::json!({
                "target": request.target,
                "identity": request.identity,
                "requires": request.requires,
                "contracts": request.contracts,
                "runtime": runtime,
                "package_fields": request.package_fields,
                "go_module": request.go_module,
            }))?;
            let result: Value =
                serde_json::from_str(&mcptype::generate_json(std::str::from_utf8(&input)?)?)?;
            let files = result["files"]
                .as_object()
                .ok_or("generated package has no files")?;
            let decoded = files
                .iter()
                .map(|(name, value)| {
                    let invalid = || {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "generated file is not base64 text",
                        )
                    };
                    let encoded = value.as_str().ok_or_else(invalid)?;
                    let bytes = STANDARD
                        .decode(encoded)
                        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                    let text = String::from_utf8(bytes)
                        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                    Ok((name.clone(), Value::String(text)))
                })
                .collect::<Result<Map<_, _>, io::Error>>()?;
            fs::write(
                output,
                serde_json::to_vec_pretty(&serde_json::json!({"files": decoded}))?,
            )?;
            Ok(0)
        }
        Operation::Check {
            request,
            verify_only,
        } => mcptype::check::run(&request, verify_only),
        Operation::Schema {
            schema: SchemaCommand::Contract,
        } => {
            print!("{}", mcptype::contract::CONTRACT_SCHEMA);
            Ok(0)
        }
    }
}

fn request_path_parent(path: &Path) -> PathBuf {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .to_path_buf()
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
