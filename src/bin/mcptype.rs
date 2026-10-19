use clap::{Parser, Subcommand};
use mcptype::{Language, Request};
use rmcp::{
    model::{ClientConfig, ProtocolVersion},
    service::{ClientInitializeError, ClientLifecycleMode, ClientServiceExt},
    transport::{StreamableHttpClientTransport, TokioChildProcess},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, io, path::Path, time::Duration};
use tokio::process::Command;

#[derive(Parser)]
#[command(version, about = "Bind MCP contracts to TypeScript and Python")]
struct Cli {
    #[command(subcommand)]
    command: Operation,
}

#[derive(Subcommand)]
enum Operation {
    Add {
        name: String,
        #[command(subcommand)]
        endpoint: Endpoint,
    },
    Compile,
    Generate,
    Check,
}

#[derive(Clone, Subcommand, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Endpoint {
    Http {
        url: String,
    },
    Stdio {
        command: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[derive(Serialize, Deserialize)]
struct Binding {
    endpoint: Endpoint,
    contract: Value,
}

#[derive(Serialize, Deserialize)]
struct Lock {
    version: u32,
    bindings: BTreeMap<String, Binding>,
}

async fn discover(endpoint: &Endpoint) -> Result<Value, Box<dyn std::error::Error>> {
    let service = ClientConfig::default().with_protocol_version(ProtocolVersion::V_2025_11_25);
    let lifecycle = ClientLifecycleMode::Auto {
        preferred_versions: vec![ProtocolVersion::LATEST],
        legacy_version: Some(ProtocolVersion::V_2025_11_25),
    };
    let client = match endpoint {
        Endpoint::Http { url } => {
            if !url.starts_with("http://") && !url.starts_with("https://") {
                return Err("expected an HTTP or HTTPS URL".into());
            }
            match service
                .clone()
                .serve_with_lifecycle(
                    StreamableHttpClientTransport::from_uri(url.clone()),
                    lifecycle,
                )
                .await
            {
                Err(ClientInitializeError::ConnectionClosed(context))
                    if context == "discover response" =>
                {
                    service
                        .serve_with_lifecycle(
                            StreamableHttpClientTransport::from_uri(url.clone()),
                            ClientLifecycleMode::Initialize,
                        )
                        .await?
                }
                result => result?,
            }
        }
        Endpoint::Stdio { command, args } => {
            let mut process = Command::new(command);
            process.args(args).kill_on_drop(true);
            service
                .serve_with_lifecycle(TokioChildProcess::new(process)?, lifecycle)
                .await?
        }
    };
    let result = client.list_all_tools().await;
    client.cancel().await?;
    Ok(json!({"tools": result?}))
}

fn artifacts(lock: &Lock) -> Result<BTreeMap<String, String>, Box<dyn std::error::Error>> {
    if lock.version != 1 {
        return Err("unsupported mcptype.lock.json version".into());
    }
    let contracts = lock
        .bindings
        .iter()
        .map(|(name, binding)| (name.clone(), binding.contract.clone()))
        .collect::<BTreeMap<_, _>>();
    let ts = mcptype::compile(Request {
        language: Language::TypeScript,
        contracts: contracts.clone(),
    })?
    .declarations;
    let py = mcptype::compile(Request {
        language: Language::Python,
        contracts,
    })?
    .declarations;
    let ts = format!("import \"mcptype\";\ndeclare module \"mcptype\" {{\n{ts}}}\n");
    let mut py = format!(
        "from contextlib import AbstractAsyncContextManager\nfrom mcp.types import InputResponses as _InputResponses, RequestParamsMeta as _RequestParamsMeta\nfrom .client import ToolResult as _ToolResult, ToolCallError as ToolCallError\n\n{py}\n"
    );
    for (index, name) in lock.bindings.keys().enumerate() {
        if lock.bindings.len() > 1 {
            py.push_str("@_typing.overload\n");
        }
        py.push_str(&format!("def mcp(name: _typing.Literal[{}], *, url: str | None = ..., headers: dict[str, str] | None = ...) -> AbstractAsyncContextManager[_App{index}]: ...\n", serde_json::to_string(name)?));
    }
    if lock.bindings.is_empty() {
        py.push_str("def mcp(name: _typing.Never) -> AbstractAsyncContextManager[_Clients]: ...\n");
    }
    Ok(BTreeMap::from([
        ("mcptype-env.d.ts".into(), ts),
        ("typings/mcptype/__init__.pyi".into(), py),
    ]))
}

fn read_lock() -> Result<Lock, Box<dyn std::error::Error>> {
    Ok(serde_json::from_slice(&fs::read("mcptype.lock.json")?)?)
}

fn write_artifacts(files: &BTreeMap<String, String>) -> std::io::Result<()> {
    for (name, text) in files {
        let path = Path::new(name);
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, text)?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {
        Operation::Add { name, endpoint } => {
            let contract =
                tokio::time::timeout(Duration::from_secs(30), discover(&endpoint)).await??;
            let mut lock = if Path::new("mcptype.lock.json").exists() {
                read_lock()?
            } else {
                Lock {
                    version: 1,
                    bindings: BTreeMap::new(),
                }
            };
            lock.bindings.insert(name, Binding { endpoint, contract });
            let files = artifacts(&lock)?;
            write_artifacts(&files)?;
            fs::write(
                "mcptype.lock.json",
                serde_json::to_string_pretty(&lock)? + "\n",
            )?;
        }
        Operation::Compile => {
            let request = serde_json::from_reader(io::stdin().lock())?;
            println!("{}", serde_json::to_string(&mcptype::compile(request)?)?);
        }
        Operation::Generate => write_artifacts(&artifacts(&read_lock()?)?)?,
        Operation::Check => {
            for (name, expected) in artifacts(&read_lock()?)? {
                if fs::read_to_string(&name)? != expected {
                    return Err(format!("{name}: stale declarations; run mcptype generate").into());
                }
            }
        }
    }
    Ok(())
}
