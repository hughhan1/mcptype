//! `mcptype check` verifies extracted sources and type-checks with the app's own `tsc` or `mypy`.
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn scratch(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("mcptype-check-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).unwrap();
    directory
}

fn hash(path: &Path) -> String {
    Sha256::digest(fs::read(path).unwrap())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Writes `entry` with `source`, and a check.json in `.holon-types/` as Holon stages it.
fn project(directory: &Path, entry: &str, source: &str) -> PathBuf {
    fs::write(directory.join(entry), source).unwrap();
    fs::create_dir_all(directory.join(".holon-types")).unwrap();
    let request = directory.join(".holon-types/check.json");
    let sources = json!({ entry: hash(&directory.join(entry)) });
    fs::write(
        &request,
        json!({"entry": entry, "sources": sources}).to_string(),
    )
    .unwrap();
    request
}

fn check(directory: &Path, request: &Path, args: &[&str], path: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_mcptype"));
    command
        .arg("check")
        .arg(request)
        .args(args)
        .current_dir(directory);
    if let Some(path) = path {
        let paths = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .collect::<Vec<_>>();
        command.env(
            "PATH",
            std::env::join_paths([path.to_path_buf()].into_iter().chain(paths)).unwrap(),
        );
    }
    command.output().unwrap()
}

fn run(directory: &Path, program: &str, args: &[&str]) {
    let result = Command::new(program)
        .args(args)
        .current_dir(directory)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{program} {args:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn verify_rejects_sources_changed_after_extraction() {
    let directory = scratch("verify");
    let request = project(&directory, "server.ts", "export const a = 1;\n");
    assert!(
        check(&directory, &request, &["--verify-only"], None)
            .status
            .success()
    );
    fs::write(directory.join("server.ts"), "export const a = 2;\n").unwrap();
    let output = check(&directory, &request, &["--verify-only"], None);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("server.ts: application install/build changed a source file")
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn typescript_checks_with_tsc() {
    let directory = scratch("typescript");
    // tsc on PATH, as when the app has no node_modules/typescript.
    let bin = directory.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let tsc = bin.join("tsc");
    fs::write(
        &tsc,
        "#!/bin/sh\nexec npx --yes -p typescript@5.9.3 tsc \"$@\"\n",
    )
    .unwrap();
    run(&bin, "chmod", &["+x", "tsc"]);
    let request = project(&directory, "server.ts", "export const total: number = 1;\n");
    let output = check(&directory, &request, &[], Some(&bin));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let request = project(
        &directory,
        "server.ts",
        "export const total: number = \"1\";\n",
    );
    let output = check(&directory, &request, &[], Some(&bin));
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("server.ts(1,14): error TS2322"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn python_checks_with_the_project_virtual_environment() {
    let directory = scratch("python");
    run(&directory, "uv", &["venv", "--quiet", ".venv"]);
    run(
        &directory,
        "uv",
        &[
            "pip",
            "install",
            "--quiet",
            "--python",
            ".venv/bin/python",
            "mypy==2.4.0",
        ],
    );
    let app = directory.join("app");
    fs::create_dir_all(&app).unwrap();
    let request = project(&app, "server.py", "total: int = 1\n");
    let output = check(&app, &request, &[], None);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let request = project(&app, "server.py", "total: int = \"1\"\n");
    let output = check(&app, &request, &[], None);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("server.py:1: error: Incompatible types in assignment")
    );
    fs::remove_dir_all(directory).unwrap();
}
