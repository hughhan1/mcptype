use base64::{Engine, engine::general_purpose::STANDARD};
use mcptype::{GenerateRequest, Target, generate};
use serde_json::json;
use std::collections::BTreeMap;

fn request(target: Target) -> GenerateRequest {
    let files = match target {
        Target::Node | Target::Browser => BTreeMap::from([
            (
                "protocol.d.ts".into(),
                "export interface ToolClient<A,B,C> {}\n".to_string(),
            ),
            (
                "runtime.mjs".into(),
                "export const runtime = true;\n".to_string(),
            ),
            (
                "browser-runtime.mjs".into(),
                "export const browser = true;\n".to_string(),
            ),
            (
                "validate.mjs".into(),
                "export const validator = () => () => true;\n".to_string(),
            ),
            ("THIRD-PARTY-NOTICES.txt".into(), "notice\n".to_string()),
        ]),
        Target::Python => [
            "__init__.py",
            "client.py",
            "binding.py",
            "context.py",
            "identity.py",
            "context.schema.json",
            "identity.schema.json",
            "py.typed",
        ]
        .into_iter()
        .map(|path| (path.to_string(), format!("# {path}\n")))
        .collect(),
        Target::Go => BTreeMap::new(),
    };
    let runtime: BTreeMap<String, String> = files
        .into_iter()
        .map(|(path, contents)| (path, STANDARD.encode(contents)))
        .collect();
    serde_json::from_value(json!({
        "target": target,
        "identity": {"name":"client", "version":"1"},
        "requires": {
            "orders":{"name":"orders", "version":"1"},
            "raw":{"name":"raw", "version":"1"}
        },
        "contracts": {
            "orders": {
                "tools":[{"name":"get_order", "inputSchema":{"type":"object", "properties":{"id":{"type":"string"}}, "required":["id"]}, "outputSchema":{"type":"object"}}],
                "resources":[{"uri":"orders://summary", "outputSchema":{"type":"object"}}]
            }
        },
        "runtime": runtime,
        "package_fields":{"holon":"holon-generated-apps-v1"},
        "go_module":"github.com/holon-sh/sdk/go"
    })).unwrap()
}

#[test]
fn generates_node_and_browser_packages_from_contracts_and_runtime_files() {
    let node = generate(request(Target::Node)).unwrap();
    let browser = generate(request(Target::Browser)).unwrap();
    for (package, required) in [
        (
            &node,
            &[
                "index.js",
                "index.d.ts",
                "resources.d.ts",
                "runtime.mjs",
                "package.json",
            ][..],
        ),
        (
            &browser,
            &[
                "browser.js",
                "browser.d.ts",
                "browser-resources.d.ts",
                "browser-runtime.mjs",
                "validate.mjs",
            ][..],
        ),
    ] {
        for path in required {
            assert!(package.files.contains_key(*path), "missing {path}");
        }
    }
    assert!(
        String::from_utf8(node.files["resources.d.ts"].clone())
            .unwrap()
            .contains("orders://summary")
    );
    let node_types = String::from_utf8(node.files["index.d.ts"].clone()).unwrap();
    assert!(node_types.contains("export interface UntypedDependency"));
    assert!(node_types.contains("readonly \"raw\": unknown"));
    assert!(
        String::from_utf8(browser.files["browser.js"].clone())
            .unwrap()
            .contains("bindApps")
    );
    assert_eq!(
        String::from_utf8(browser.files["validate.mjs"].clone()).unwrap(),
        "export const validator = () => () => true;\n"
    );
    assert_eq!(
        String::from_utf8(node.files["runtime.mjs"].clone()).unwrap(),
        "export const runtime = true;\n"
    );
}

#[test]
fn generates_python_and_go_packages_with_untyped_dependencies() {
    let python = generate(request(Target::Python)).unwrap();
    assert!(python.files.contains_key("_apps.py"));
    assert!(python.files.contains_key("client.py"));
    assert!(
        String::from_utf8(python.files["_apps.py"].clone())
            .unwrap()
            .contains("create_apps")
    );
    let python_types = String::from_utf8(python.files["_apps.py"].clone()).unwrap();
    assert!(python_types.contains("raw: _UntypedClient"));

    let go = generate(request(Target::Go)).unwrap();
    let clients = String::from_utf8(go.files["clients.go"].clone()).unwrap();
    assert!(clients.contains("func (client _Raw) CallTool"));
    assert!(clients.contains("func (client _Orders) GetOrder"));
    assert!(clients.contains("github.com/holon-sh/sdk/go"));
}

#[test]
fn typed_and_untyped_go_aliases_share_the_compilers_unique_symbols() {
    let mut request = request(Target::Go);
    request.requires.insert(
        "orders-".into(),
        mcptype::Identity {
            name: "raw-orders".into(),
            version: "1".into(),
        },
    );
    let generated = generate(request).unwrap();
    let clients = String::from_utf8(generated.files["clients.go"].clone()).unwrap();
    assert!(clients.contains("type _Orders struct"));
    assert!(clients.contains("func (client _Orders) GetOrder"));
    assert!(clients.contains("type _Orders2 struct"));
    assert!(clients.contains("func (client _Orders2) CallTool"));
}

#[test]
fn runtime_file_paths_are_required_by_the_selected_generator() {
    let mut request = request(Target::Node);
    request.runtime.remove("runtime.mjs");
    let error = generate(request).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("missing runtime file runtime.mjs")
    );
}
