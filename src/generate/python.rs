use super::{GenerateRequest, compile_contracts, put};
use crate::{Error, Language};
use serde_json::Value;
use std::{collections::BTreeMap, sync::atomic::AtomicBool};

const RUNTIME_FILES: &[&str] = &[
    "__init__.py",
    "binding.py",
    "client.py",
    "context.py",
    "identity.py",
    "context.schema.json",
    "identity.schema.json",
    "py.typed",
];

fn python_identifier(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|value| value.is_ascii_alphabetic() || value == '_')
        && name
            .chars()
            .all(|value| value.is_ascii_alphanumeric() || value == '_')
        && ![
            "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
            "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global",
            "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise",
            "return", "try", "while", "with", "yield",
        ]
        .contains(&name)
}

fn clients_type(request: &GenerateRequest) -> String {
    let mut members = Vec::new();
    let mut overloads = Vec::new();
    let untyped_count = request
        .requires
        .keys()
        .filter(|alias| !request.contracts.contains_key(*alias))
        .count();
    let has_multiple_apps = request.contracts.len() + untyped_count > 1;
    for (index, alias) in request.contracts.keys().enumerate() {
        if python_identifier(alias) {
            members.push(format!("    {alias}: _App{index}"));
        }
        overloads.push(format!(
            "    {}def __getitem__(self, name: _typing.Literal[{}]) -> _App{index}: ...",
            if has_multiple_apps {
                "@_typing.overload\n    "
            } else {
                ""
            },
            serde_json::to_string(alias).unwrap()
        ));
    }
    for alias in request
        .requires
        .keys()
        .filter(|alias| !request.contracts.contains_key(*alias))
    {
        if python_identifier(alias) {
            members.push(format!("    {alias}: _UntypedClient"));
        }
        overloads.push(format!(
            "    {}def __getitem__(self, name: _typing.Literal[{}]) -> _UntypedClient: ...",
            if has_multiple_apps {
                "@_typing.overload\n    "
            } else {
                ""
            },
            serde_json::to_string(alias).unwrap()
        ));
    }
    members.extend(overloads);
    if members.is_empty() {
        members.push("    pass".into());
    }
    format!(
        "class _UntypedClient(_typing.Protocol):\n    async def call_tool(self, name: _builtins.str, arguments: _typing.Mapping[_builtins.str, _builtins.object], **options: _typing.Any) -> _ToolResult[_typing.Any]: ...\n\nclass _Clients(_typing.Protocol):\n{}\n",
        members.join("\n")
    )
}

pub(crate) fn generate(
    request: &GenerateRequest,
    runtime: &BTreeMap<String, Vec<u8>>,
    cancelled: &AtomicBool,
) -> Result<BTreeMap<String, Vec<u8>>, Error> {
    let generated = compile_contracts(&request.contracts, Language::Python, cancelled)?;
    let payload = serde_json::to_string(&serde_json::json!([request.identity, request.contracts]))?;
    let module = format!(
        "from __future__ import annotations\n\nimport builtins as _builtins\nimport json as _json\nimport typing as _typing\n\nfrom mcp.types import InputResponses as _InputResponses\nfrom mcp.types import RequestParamsMeta as _RequestParamsMeta\n\nfrom .binding import Binding as _Binding\nfrom .client import Apps as _Apps\nfrom .client import ToolResult as _ToolResult\n\n{}\n{}\n_identity, _contracts = _json.loads({:?})\napps = _typing.cast(_Clients, _Apps(_identity, _contracts))\n\ndef create_apps(bindings: _typing.Mapping[str, _Binding]) -> _Clients:\n    return _typing.cast(_Clients, _Apps(_identity, _contracts, bindings))\n",
        generated.declarations,
        clients_type(request),
        payload
    );
    let mut files = BTreeMap::new();
    for path in RUNTIME_FILES {
        files.insert(
            (*path).into(),
            runtime
                .get(*path)
                .cloned()
                .ok_or_else(|| Error::Contract(format!("missing runtime file {path}")))?,
        );
    }
    put(&mut files, "_apps.py", module);
    let marker = request
        .package_fields
        .get("holon")
        .cloned()
        .unwrap_or_else(|| Value::String("holon-generated-apps-v1".into()));
    files.insert(
        "package.json".into(),
        serde_json::to_vec(&serde_json::json!({"holon": marker}))?,
    );
    Ok(files)
}
