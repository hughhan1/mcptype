//! The generated `holon` Python package: `_apps.py` beside the runtime files.
use super::{Caller, GenerateRequest, declarations};
use crate::{Error, Language, python::identifier};
use serde_json::Value;
use std::collections::BTreeMap;

const HEADER: &str = "from __future__ import annotations

import json as _json

from mcp.types import InputResponses as _InputResponses
from mcp.types import RequestParamsMeta as _RequestParamsMeta

from .client import Apps as _Apps
from .client import Binding as _Binding
from .client import ToolResult as _ToolResult";

const UNTYPED: &str = "class _Untyped(_typing.Protocol):
    async def call_tool(self, name: _builtins.str, arguments: _typing.Mapping[_builtins.str, _builtins.object], *, timeout: _builtins.float | None = ..., input_responses: _InputResponses | None = ..., request_state: _builtins.str | None = ..., meta: _RequestParamsMeta | None = ...) -> _ToolResult[_builtins.object]: ...";

pub(super) fn generate<'a>(
    request: &'a GenerateRequest,
    caller: &Caller,
) -> Result<BTreeMap<&'a str, String>, Error> {
    let declarations = declarations(Language::Python, &request.contracts)?.declarations;
    let payload = ascii(&serde_json::to_string(&(caller, &request.contracts))?);
    let untyped: Vec<_> = request
        .untyped()
        .filter(|alias| identifier(alias))
        .map(|alias| format!("\n    {alias}: _Untyped"))
        .collect();
    let (clients, untyped) = if untyped.is_empty() {
        ("_Clients", String::new())
    } else {
        let members = untyped.concat();
        let clients = format!("class _AllClients(_Clients, _typing.Protocol):{members}");
        ("_AllClients", format!("\n{UNTYPED}\n\n{clients}\n"))
    };
    let header = HEADER.replace(
        "from .client import",
        &format!(
            "from {} import",
            request
                .python_runtime_module
                .as_deref()
                .unwrap_or(".client")
        ),
    );
    let module = [
        header.as_str(),
        &(declarations + &untyped),
        &format!("_identity, _contracts = _json.loads({})", repr(&payload)),
        &format!("apps = _typing.cast({clients}, _Apps(_identity, _contracts))"),
        &format!("def create_apps(bindings: _typing.Mapping[str, _Binding]) -> {clients}:\n    return _typing.cast({clients}, _Apps(_identity, _contracts, bindings))"),
    ]
    .join("\n\n");
    let mut files = BTreeMap::from([(
        request.python_module.as_deref().unwrap_or("_apps.py"),
        module + "\n",
    )]);
    if !request.slot() {
        files.insert(
            "package.json",
            dumps(&Value::Object(request.package_fields.clone())),
        );
    }
    Ok(files)
}

/// Python's `json.dumps` with `ensure_ascii` and its default separators.
fn dumps(value: &Value) -> String {
    match value {
        Value::Array(items) => {
            let items: Vec<_> = items.iter().map(dumps).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Object(fields) => {
            let fields: Vec<_> = fields
                .iter()
                .map(|(name, value)| format!("{}: {}", quote(name), dumps(value)))
                .collect();
            format!("{{{}}}", fields.join(", "))
        }
        Value::String(text) => quote(text),
        scalar => scalar.to_string(),
    }
}

fn quote(text: &str) -> String {
    ascii(&serde_json::to_string(text).unwrap())
}

/// JSON text with every non-ASCII character as `\u` escapes, as `ensure_ascii` writes it.
fn ascii(json: &str) -> String {
    let mut quoted = String::new();
    for unit in json.chars() {
        if unit.is_ascii() {
            quoted.push(unit);
        } else {
            let mut buffer = [0; 2];
            for unit in unit.encode_utf16(&mut buffer) {
                quoted += &format!("\\u{unit:04x}");
            }
        }
    }
    quoted
}

/// Python's `repr` of an ASCII string.
fn repr(text: &str) -> String {
    let quote = if text.contains('\'') && !text.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut literal = String::from(quote);
    for ch in text.chars() {
        match ch {
            '\\' => literal += "\\\\",
            '\n' => literal += "\\n",
            '\r' => literal += "\\r",
            '\t' => literal += "\\t",
            ch if ch == quote => {
                literal.push('\\');
                literal.push(ch);
            }
            ch if (ch as u32) < 0x20 || ch == '\x7f' => literal += &format!("\\x{:02x}", ch as u32),
            ch => literal.push(ch),
        }
    }
    literal.push(quote);
    literal
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_and_repr_match_python() {
        let value = json!({"a": ["é😀", 1, 2.5, null, true], "b'": "\\\n"});
        assert_eq!(
            dumps(&value),
            r#"{"a": ["|u00e9|ud83d|ude00", 1, 2.5, null, true], "b'": "\\\n"}"#.replace('|', "\\")
        );
        assert_eq!(ascii("\"é\""), r#""|u00e9""#.replace('|', "\\"));
        assert_eq!(repr(r#"{"b'":"\\n"}"#), r#"'{"b\'":"\\\\n"}'"#);
        assert_eq!(repr("it's"), r#""it's""#);
    }
}
