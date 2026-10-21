use crate::{Error, Identity, Request, compile_cancellable};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{collections::BTreeMap, sync::atomic::AtomicBool};

mod go;
mod python;
mod typescript;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    Node,
    Browser,
    Python,
    Go,
}

#[derive(Debug, Deserialize)]
pub struct GenerateRequest {
    pub target: Target,
    pub identity: Identity,
    #[serde(default)]
    pub requires: BTreeMap<String, Identity>,
    #[serde(default)]
    pub contracts: BTreeMap<String, crate::Contract>,
    /// Runtime file contents encoded as base64 at the language boundary.
    #[serde(default)]
    pub runtime: BTreeMap<String, String>,
    #[serde(default)]
    pub package_fields: Map<String, Value>,
    pub go_module: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GeneratedPackage {
    pub files: BTreeMap<String, Vec<u8>>,
}

pub fn generate(request: GenerateRequest) -> Result<GeneratedPackage, Error> {
    generate_cancellable(request, &AtomicBool::new(false))
}

pub fn generate_cancellable(
    request: GenerateRequest,
    cancelled: &AtomicBool,
) -> Result<GeneratedPackage, Error> {
    crate::check_cancelled(cancelled)?;
    if request.identity.name.is_empty() || request.identity.version.is_empty() {
        return Err(Error::Contract("invalid generated package identity".into()));
    }
    for (alias, identity) in &request.requires {
        if alias.is_empty() || identity.name.is_empty() || identity.version.is_empty() {
            return Err(Error::Contract(
                "invalid generated dependency identity".into(),
            ));
        }
    }
    for contract in request.contracts.values() {
        for (name, tool) in &contract.tools {
            if name.is_empty() || tool.input.get("type") != Some(&Value::from("object")) {
                return Err(Error::Contract(
                    "tool names must be nonempty and input schemas must be objects".into(),
                ));
            }
        }
        if contract.resources.keys().any(String::is_empty) {
            return Err(Error::Contract("resource URIs must be nonempty".into()));
        }
    }
    if let Some(module) = &request.go_module
        && module.trim().is_empty()
    {
        return Err(Error::Contract("go_module must be nonempty".into()));
    }
    let runtime = request
        .runtime
        .iter()
        .map(|(path, encoded)| {
            STANDARD
                .decode(encoded)
                .map(|bytes| (path.clone(), bytes))
                .map_err(|error| {
                    Error::Contract(format!("invalid base64 runtime file {path}: {error}"))
                })
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;

    let files = match request.target {
        Target::Node | Target::Browser => typescript::generate(&request, &runtime, cancelled)?,
        Target::Python => python::generate(&request, &runtime, cancelled)?,
        Target::Go => go::generate(&request, cancelled)?,
    };
    Ok(GeneratedPackage { files })
}

/// JSON entry point used by the C ABI and the Node/Python native bindings.
/// File bytes are encoded as base64 in the returned JSON object.
pub fn generate_json(input: &str) -> Result<String, Error> {
    let request: GenerateRequest = serde_json::from_str(input)?;
    let generated = generate(request)?;
    let files = generated
        .files
        .into_iter()
        .map(|(path, bytes)| (path, Value::String(STANDARD.encode(bytes))))
        .collect::<Map<_, _>>();
    Ok(serde_json::to_string(&serde_json::json!({"files": files}))?)
}

pub(crate) fn compile_contracts(
    contracts: &BTreeMap<String, crate::Contract>,
    language: crate::Language,
    cancelled: &AtomicBool,
) -> Result<crate::Generated, Error> {
    compile_cancellable(
        Request {
            language,
            contracts: contracts.clone(),
        },
        cancelled,
    )
}

pub(crate) fn compile_resources(
    contracts: &BTreeMap<String, crate::Contract>,
    cancelled: &AtomicBool,
) -> Result<crate::Generated, Error> {
    let resource_contracts = contracts
        .iter()
        .map(|(name, contract)| {
            let tools = contract
                .resources
                .iter()
                .map(|(uri, resource)| {
                    let input = serde_json::json!({
                        "type": "object",
                        "additionalProperties": false,
                    })
                    .as_object()
                    .expect("JSON objects deserialize as maps")
                    .clone();
                    (
                        uri.clone(),
                        crate::Tool {
                            input,
                            output: Some(resource.output.clone()),
                        },
                    )
                })
                .collect();
            (
                name.clone(),
                crate::Contract {
                    tools,
                    resources: BTreeMap::new(),
                },
            )
        })
        .collect();
    compile_cancellable(
        Request {
            language: crate::Language::TypeScript,
            contracts: resource_contracts,
        },
        cancelled,
    )
}

pub(crate) fn package_json(request: &GenerateRequest) -> Result<Vec<u8>, Error> {
    let mut fields = serde_json::json!({
        "name": "@holon-sh/apps",
        "version": "0.0.0",
        "private": true,
        "type": "module",
    })
    .as_object()
    .unwrap()
    .clone();
    fields.extend(request.package_fields.clone());
    fields.insert(
        "exports".into(),
        serde_json::json!({
            ".": {"types":"./index.d.ts", "import":"./index.js"},
            "./browser": {"types":"./browser.d.ts", "import":"./browser.js"}
        }),
    );
    Ok(serde_json::to_vec_pretty(&Value::Object(fields))?)
}

pub(crate) fn runtime_text(
    runtime: &BTreeMap<String, Vec<u8>>,
    path: &str,
) -> Result<String, Error> {
    let bytes = runtime
        .get(path)
        .ok_or_else(|| Error::Contract(format!("missing runtime file {path}")))?;
    String::from_utf8(bytes.clone())
        .map_err(|error| Error::Contract(format!("runtime file {path} is not UTF-8: {error}")))
}

pub(crate) fn put(files: &mut BTreeMap<String, Vec<u8>>, path: &str, contents: String) {
    files.insert(path.into(), contents.into_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn package_fields_extend_package_metadata() {
        let request: GenerateRequest = serde_json::from_value(json!({
            "target":"node", "identity":{"name":"app","version":"1"},
            "package_fields":{"holon":"holon-generated-apps-v1"}
        }))
        .unwrap();
        let value: Value = serde_json::from_slice(&package_json(&request).unwrap()).unwrap();
        assert_eq!(value["holon"], "holon-generated-apps-v1");
        assert_eq!(value["type"], "module");
    }
}
