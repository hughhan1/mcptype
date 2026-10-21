//! `mcptype check`: the type check a generated package's caller runs, without the SDK's runners.
//!
//! The request is the runners' `check.json`. Paths are relative to `root`, which defaults to the
//! working directory: verify that each extracted source still has the hash it had when its contract
//! was extracted, then type-check the app with its own `tsc` or `mypy`.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckRequest {
    /// `typescript` or `python`; inferred from `entry` when absent.
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    root: Option<PathBuf>,
    #[serde(default)]
    entry: Option<PathBuf>,
    /// Hex SHA-256 of each source the contract was extracted from.
    #[serde(default)]
    sources: BTreeMap<PathBuf, String>,
    /// Whether to type-check after verifying sources.
    #[serde(default = "enabled")]
    check: bool,
}

fn enabled() -> bool {
    true
}

/// Runs the check and returns the type checker's exit status (0 when only verifying).
pub fn run(request: &Path, verify_only: bool) -> Result<i32, Box<dyn Error>> {
    let request: CheckRequest = serde_json::from_slice(&fs::read(request)?)?;
    let root = request.root.clone().unwrap_or_else(|| PathBuf::from("."));
    for (path, expected) in &request.sources {
        let actual = hex(&Sha256::digest(fs::read(root.join(path))?));
        if actual != *expected {
            return Err(format!(
                "{}: application install/build changed a source file used for its MCP contract; refusing to publish stale types.",
                path.display()
            )
            .into());
        }
    }
    if verify_only || !request.check {
        return Ok(0);
    }
    let python = request
        .language
        .as_deref()
        .map(|language| language == "python")
        .unwrap_or_else(|| {
            request
                .entry
                .as_ref()
                .is_some_and(|entry| entry.extension().is_some_and(|ext| ext == "py"))
        });
    let mut command = if python {
        python_check(&root, &request)?
    } else {
        typescript_check(&root, &request)?
    };
    Ok(command.current_dir(&root).status()?.code().unwrap_or(1))
}

/// The app's own `tsc` (from `node_modules` at or above root) or the one on `PATH`, with its
/// `tsconfig.json` or the runners' defaults.
fn typescript_check(root: &Path, request: &CheckRequest) -> Result<Command, Box<dyn Error>> {
    let root = root.canonicalize()?;
    let local = root
        .ancestors()
        .map(|directory| directory.join("node_modules/typescript/bin/tsc"))
        .find(|tsc| tsc.is_file());
    let mut command = match local {
        Some(tsc) => {
            let mut command = Command::new("node");
            command.arg(tsc);
            command
        }
        None => Command::new("tsc"),
    };
    command.arg("--noEmit");
    if root.join("tsconfig.json").is_file() {
        command.args(["--project", "tsconfig.json"]);
    } else {
        let entry = request
            .entry
            .as_ref()
            .ok_or("a TypeScript check needs a tsconfig.json or an entry")?;
        command
            .args([
                "--strict",
                "--skipLibCheck",
                "--allowJs",
                "--target",
                "ES2023",
                "--module",
                "NodeNext",
                "--moduleResolution",
                "NodeNext",
                "--allowImportingTsExtensions",
            ])
            .arg(entry);
    }
    Ok(command)
}

/// `mypy` run by the nearest virtual environment's Python at or above root, or by `python3`.
fn python_check(root: &Path, request: &CheckRequest) -> Result<Command, Box<dyn Error>> {
    let entry = request
        .entry
        .as_ref()
        .ok_or("a Python check needs an entry")?;
    let root = root.canonicalize()?;
    let python = root
        .ancestors()
        .flat_map(|directory| [".venv", "venv"].map(|venv| directory.join(venv).join("bin/python")))
        .find(|python| python.is_file())
        .unwrap_or_else(|| PathBuf::from("python3"));
    let mut command = Command::new(python);
    command
        .args(["-m", "mypy", "--no-incremental", "--check-untyped-defs"])
        .arg(entry);
    Ok(command)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
