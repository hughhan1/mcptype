use base64::{Engine, engine::general_purpose::STANDARD};
use clap::{Parser, Subcommand};
use mcptype::{Contract, Expectation, Identity, Metadata, Request, changes};
use serde::Deserialize;
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
enum Cli {
    /// Compile a request read from stdin.
    Compile,
    /// Print what breaks a caller's expectation if the published metadata is deployed.
    Diff {
        expected: PathBuf,
        published: PathBuf,
    },
    /// Generate package files from a request whose runtime values name files on disk.
    Generate { request: PathBuf, output: PathBuf },
    /// Verify source fingerprints and run the local language type checker.
    Check {
        request: PathBuf,
        #[arg(long)]
        verify_only: bool,
    },
    /// Print the published contract JSON Schema.
    Schema {
        #[command(subcommand)]
        schema: SchemaCommand,
    },
}

#[derive(Deserialize)]
struct FileGenerateRequest {
    target: mcptype::Target,
    identity: Identity,
    #[serde(default)]
    requires: BTreeMap<String, Identity>,
    #[serde(default)]
    contracts: BTreeMap<String, Contract>,
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
    match Cli::parse() {
        Cli::Compile => {
            let request: Request = serde_json::from_reader(io::stdin().lock())?;
            println!("{}", serde_json::to_string(&mcptype::compile(request)?)?);
            Ok(0)
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
            Ok(i32::from(!changes.is_empty()))
        }
        Cli::Generate { request, output } => {
            let request_path = request.clone();
            let mut request: FileGenerateRequest =
                serde_json::from_slice(&fs::read(&request_path)?)?;
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
                    let encoded = value.as_str().ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "generated file is not base64 text",
                        )
                    })?;
                    let bytes = STANDARD
                        .decode(encoded)
                        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                    let contents = String::from_utf8(bytes)
                        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                    Ok((name.clone(), Value::String(contents)))
                })
                .collect::<Result<Map<_, _>, io::Error>>()?;
            fs::write(
                output,
                serde_json::to_vec_pretty(&serde_json::json!({"files": decoded}))?,
            )?;
            Ok(0)
        }
        Cli::Check {
            request,
            verify_only,
        } => mcptype::check::run(&request, verify_only),
        Cli::Schema {
            schema: SchemaCommand::Contract,
        } => {
            print!("{}", include_str!("../../schema/contract.schema.json"));
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
