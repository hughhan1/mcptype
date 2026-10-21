use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    env,
    error::Error,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Deserialize)]
pub struct CheckRequest {
    pub language: String,
    #[serde(default)]
    pub root: Option<PathBuf>,
    #[serde(default)]
    pub entry: Option<PathBuf>,
    #[serde(default)]
    pub sources: BTreeMap<PathBuf, String>,
    #[serde(default = "enabled")]
    pub check: bool,
}

fn enabled() -> bool {
    true
}

pub fn run(request: &Path, verify_only: bool) -> Result<i32, Box<dyn Error>> {
    let request_root = request
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let config: CheckRequest = serde_json::from_slice(&fs::read(request)?)?;
    let root = config
        .root
        .map(|root| {
            if root.is_absolute() {
                root
            } else {
                request_root.join(root)
            }
        })
        .unwrap_or_else(|| request_root.to_path_buf());

    for (path, expected) in &config.sources {
        let contents = fs::read(root.join(path))?;
        let actual = Sha256::digest(&contents)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let expected = expected.strip_prefix("sha256:").unwrap_or(expected);
        if actual != expected {
            return Err(format!(
                "{}: application install/build changed a contract source; refusing to publish stale types.",
                path.display()
            )
            .into());
        }
    }
    if verify_only || !config.check {
        return Ok(0);
    }

    let mut command = match config.language.as_str() {
        "typescript" | "node" | "browser" => {
            let local_tsc = root.join("node_modules/typescript/bin/tsc");
            let mut command = if local_tsc.is_file() {
                let mut command = Command::new("node");
                command.arg(local_tsc);
                command
            } else {
                Command::new("tsc")
            };
            command.current_dir(&root).arg("--noEmit");
            let tsconfig = root.join("tsconfig.json");
            if tsconfig.is_file() {
                command.args(["--project", "tsconfig.json"]);
            } else if let Some(entry) = &config.entry {
                command.arg(entry);
            } else {
                return Err("TypeScript check requires tsconfig.json or an entry".into());
            }
            command
        }
        "python" => {
            let entry = config
                .entry
                .as_ref()
                .ok_or("Python check requires an entry")?;
            let mut command = Command::new("mypy");
            command
                .current_dir(&root)
                .args(["--no-incremental", "--check-untyped-defs"])
                .arg(entry);
            command
        }
        language => return Err(format!("unsupported check language {language:?}").into()),
    };
    command.env("PATH", local_tool_path(&root)?);
    Ok(command.status()?.code().unwrap_or(2))
}

fn local_tool_path(root: &Path) -> Result<OsString, Box<dyn Error>> {
    let mut paths = Vec::new();
    for relative in ["node_modules/.bin", ".venv/bin", "venv/bin"] {
        let path = root.join(relative);
        if path.is_dir() {
            paths.push(path);
        }
    }
    #[cfg(windows)]
    for relative in [".venv/Scripts", "venv/Scripts"] {
        let path = root.join(relative);
        if path.is_dir() {
            paths.push(path);
        }
    }
    paths.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()));
    Ok(env::join_paths(paths)?)
}
