//! Generated packages type-check with each language's own tools: `tsc`, `mypy --strict`, `go vet` and `gofmt`.
use base64::{Engine, engine::general_purpose::STANDARD};
use mcptype::{GeneratedPackage, generate};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const PROTOCOL: &str = "type AuthProvider = { token(): Promise<string> };
type ToolClient<I, O, R> = { tools: { [K in keyof I]: (input: I[K]) => Promise<K extends keyof O ? O[K] : unknown> }; resources: { [U in keyof R]: { read(): Promise<R[U]> } }; callTool<K extends keyof I & string>(request: { name: K; arguments: I[K] }): Promise<unknown> };";

fn contracts() -> Value {
    let order = json!({"tools": [
        {"name": "get_order", "inputSchema": {"type": "object", "additionalProperties": false,
            "properties": {"orderId": {"type": "string"}, "count": {"type": "integer"}}, "required": ["orderId"]},
         "outputSchema": {"type": "object", "properties": {"total": {"type": "number"}}, "required": ["total"]}},
        {"name": "then", "inputSchema": {"type": "object", "additionalProperties": false}}
    ], "resources": [{"uri": "orders://summary", "outputSchema": {"type": "object", "properties": {"total": {"type": "number"}}, "required": ["total"]}}]});
    let nested: Value = serde_json::from_str(include_str!("vectors/nested-types.json")).unwrap();
    json!({"orders": order, "with-hyphen": order, "mysql": nested, "empty": {"tools": []}})
}

fn package(target: &str, runtime: &[(&str, &str)]) -> BTreeMap<String, Vec<u8>> {
    let runtime: BTreeMap<_, _> = runtime
        .iter()
        .map(|(path, text)| (*path, STANDARD.encode(text)))
        .collect();
    let request = json!({
        "target": target,
        "identity": {"name": "checkout", "version": "1.0.0"},
        "requires": {"orders": {"name": "orders", "version": "1.0.0"}, "payments": {"name": "payments", "version": "2.0.0"}},
        "contracts": contracts(),
        "runtime": runtime,
        "protocol": PROTOCOL,
        "package_fields": {"holon": "holon-generated-apps-v1"},
        "go_module": "example.com/runtime",
    });
    let GeneratedPackage { files } = generate(serde_json::from_value(request).unwrap()).unwrap();
    files
}

fn write(directory: &Path, files: &BTreeMap<String, Vec<u8>>) {
    fs::create_dir_all(directory).unwrap();
    for (path, data) in files {
        fs::write(directory.join(path), data).unwrap();
    }
}

fn scratch(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("mcptype-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).unwrap();
    directory
}

fn run(directory: &Path, program: &str, args: &[&str]) -> String {
    let result = Command::new(program)
        .args(args)
        .current_dir(directory)
        .env("GOWORK", "off")
        .output()
        .unwrap_or_else(|error| {
            panic!("{program} is required for generated package checks: {error}")
        });
    let output = String::from_utf8_lossy(&result.stdout).into_owned()
        + &String::from_utf8_lossy(&result.stderr);
    assert!(result.status.success(), "{program} {args:?}\n{output}");
    output
}

#[test]
fn typescript_packages_type_check() {
    let runtime = [
        ("runtime.mjs", "export {};\n"),
        ("browser-runtime.mjs", "export {};\n"),
    ];
    let node = package("node", &runtime);
    let browser = package("browser", &runtime);
    assert_eq!(node["package.json"], browser["package.json"]);
    assert_eq!(node["runtime.mjs"], b"export {};\n");
    let package_json: Value = serde_json::from_slice(&node["package.json"]).unwrap();
    assert_eq!(package_json["holon"], "holon-generated-apps-v1");
    let index = String::from_utf8_lossy(&node["index.js"]).into_owned();
    assert!(
        index.contains(r#""requires":{"orders":{"name":"orders","version":"1.0.0"},"payments""#),
        "{index}"
    );

    let directory = scratch("typescript");
    let apps = directory.join("node_modules/@holon-sh/apps");
    write(&apps, &node);
    write(&apps, &browser);
    fs::write(directory.join("package.json"), r#"{"type":"module"}"#).unwrap();
    fs::write(directory.join("check.ts"), r#"import { apps, createApps as createNodeApps, type Inputs, type Resources } from "@holon-sh/apps";
import { createApps, type CreateOptions } from "@holon-sh/apps/browser";
const order: Inputs["orders"]["get_order"] = { orderId: "a", count: 1 };
// @ts-expect-error
const wrong: Inputs["orders"]["get_order"] = { orderId: 1 };
(await apps.orders.tools.get_order(order)).total.toFixed();
const summary: Resources["orders"]["orders://summary"] = await apps.orders.resources["orders://summary"].read();
summary.total.toFixed();
const options: CreateOptions = { validator: () => () => true };
createApps({ orders: { endpoint: "https://orders.example/mcp" } }, options);
createNodeApps({ mysql: { endpoint: new URL("https://mysql.example/mcp") } });
// @ts-expect-error
createApps({ missing: { endpoint: "https://missing.example/mcp" } });
apps.empty;
await apps.payments.callTool({ name: "charge", arguments: { amount: 1 } });
// @ts-expect-error
apps.payments.tools.charge;
// @ts-expect-error
apps.payments.resources["payments://summary"];
createApps({ payments: { endpoint: "https://payments.example/mcp" } }).payments.callTool({ name: "x", arguments: {} });
"#).unwrap();
    run(
        &directory,
        "npx",
        &[
            "--yes",
            "-p",
            "typescript@5.9.3",
            "tsc",
            "--noEmit",
            "--strict",
            "--target",
            "ES2023",
            "--module",
            "NodeNext",
            "--moduleResolution",
            "NodeNext",
            "check.ts",
        ],
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn python_package_type_checks() {
    let files = package("python", &[("py.typed", "")]);
    assert_eq!(
        files["package.json"],
        br#"{"holon": "holon-generated-apps-v1"}"#
    );
    let directory = scratch("python");
    write(&directory.join("holon"), &files);
    fs::write(directory.join("holon/__init__.py"), "").unwrap();
    fs::write(directory.join("holon/client.py"), r#"from collections.abc import Mapping
from typing import Any, Generic, TypedDict, TypeVar

Output = TypeVar("Output")


class Binding(TypedDict):
    endpoint: str


class ToolResult(Generic[Output]):
    pass


class Apps:
    def __init__(self, identity: Any, contracts: Any, bindings: Mapping[str, Binding] | None = None) -> None:
        pass
"#).unwrap();
    fs::create_dir_all(directory.join("mcp")).unwrap();
    fs::write(directory.join("mcp/__init__.py"), "").unwrap();
    fs::write(
        directory.join("mcp/types.py"),
        "class InputResponses:\n    pass\n\n\nclass RequestParamsMeta:\n    pass\n",
    )
    .unwrap();
    fs::write(
        directory.join("check.py"),
        r#"from holon._apps import apps, create_apps


async def main() -> None:
    result = await apps.orders.tools.get_order({"orderId": "a"})
    total: float = result["total"]
    create_apps({"orders": {"endpoint": "https://orders.example/mcp"}})
    await apps.orders.tools.get_order({"orderId": 1})  # type: ignore[typeddict-item]
    charged = await apps.payments.call_tool("charge", {"amount": 1})
    apps.payments.tools  # type: ignore[attr-defined]
    print(total, charged)
"#,
    )
    .unwrap();
    run(
        &directory,
        "uv",
        &[
            "run",
            "--no-project",
            "--with",
            "mypy==2.4.0",
            "mypy",
            "--strict",
            "--warn-unused-ignores",
            "check.py",
            "holon/_apps.py",
        ],
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn go_package_vets_and_clients_are_gofmt_clean() {
    let files = package("go", &[]);
    assert_eq!(
        files["package.json"],
        br#"{"holon":"holon-generated-apps-v1"}"#
    );
    let directory = scratch("go");
    write(&directory.join("holon"), &files);
    fs::write(directory.join("go.mod"), "module example.com/app\n\ngo 1.24\n\nrequire example.com/runtime v0.0.0\n\nreplace example.com/runtime => ./runtime\n").unwrap();
    fs::create_dir_all(directory.join("runtime")).unwrap();
    fs::write(
        directory.join("runtime/go.mod"),
        "module example.com/runtime\n\ngo 1.24\n",
    )
    .unwrap();
    fs::write(directory.join("runtime/runtime.go"), r#"package holon

import "context"

type ToolSchema struct{ Input, Output string }

type Dependency struct {
	Name  string
	Tools map[string]ToolSchema
}

func Call[Output any](ctx context.Context, dependency *Dependency, name string, input any) (Output, error) {
	var output Output
	return output, nil
}
"#).unwrap();
    fs::write(
        directory.join("main.go"),
        r#"package main

import (
	"context"

	"example.com/app/holon"
)

func main() {
	output, err := holon.Orders.GetOrder(context.Background(), holon.OrdersGetOrderInput{OrderId: "a"})
	_, _ = output["total"], err
	_ = holon.Empty
}
"#,
    )
    .unwrap();
    run(&directory, "go", &["vet", "./..."]);
    // Declarations come unchanged from `compile`; the package layout around them is gofmt-clean.
    assert_eq!(
        run(&directory, "gofmt", &["-l", "holon/clients.go", "main.go"]),
        ""
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn requests_are_validated() {
    for (patch, message) in [
        (
            json!({"target": "go", "go_module": null}),
            "go_module is required",
        ),
        (json!({"runtime": {"../escape": ""}}), "must be relative"),
        (json!({"runtime": {"file": "%%"}}), "runtime file"),
        (
            json!({"contracts": {"": {"tools": []}}}),
            "binding names must be nonempty",
        ),
    ] {
        let mut request = json!({"target": "node", "identity": {"name": "a", "version": "1"}, "go_module": "example.com/runtime"});
        request
            .as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        let error = serde_json::from_value(request)
            .map_err(mcptype::Error::from)
            .and_then(generate)
            .unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
    }
}
