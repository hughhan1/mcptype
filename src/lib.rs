pub mod canonical;
pub mod changes;
pub mod contract;
mod go;
mod python;
mod schema;
mod typescript;

pub use canonical::{canonical, canonical_json, digest};
pub use changes::{Change, changes, changes_json};
pub use contract::{Contract, Expectation, Identity, Metadata, Resource, Schema, Tool};
pub use go::{Binding as GoBinding, Tool as GoTool};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

fn quoted(value: &str) -> String {
    serde_json::to_string(value).unwrap()
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("compilation cancelled")]
    Cancelled,
    #[error("{0}")]
    Contract(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    TypeScript,
    Python,
    Go,
}

impl Language {
    pub const ALL: [Self; 3] = [Self::TypeScript, Self::Python, Self::Go];

    /// Where the CLI writes this language's declarations, relative to `mcptype.lock.json`.
    pub fn path(self) -> &'static str {
        match self {
            Self::TypeScript => "mcptype-env.d.ts",
            Self::Python => "typings/mcptype/__init__.pyi",
            Self::Go => "mcptype-env.go",
        }
    }

    fn generate(
        self,
        contracts: &BTreeMap<String, Catalog>,
    ) -> (String, Option<BTreeMap<String, GoBinding>>) {
        match self {
            Self::TypeScript => (typescript::generate(contracts), None),
            Self::Python => (python::generate(contracts), None),
            Self::Go => {
                let (declarations, bindings) = go::generate(contracts);
                (declarations, Some(bindings))
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Request {
    pub language: Language,
    pub contracts: BTreeMap<String, Value>,
}

#[derive(Debug, Serialize)]
pub struct Generated {
    pub declarations: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bindings: Option<BTreeMap<String, GoBinding>>,
}

struct Catalog {
    tools: BTreeMap<String, CompiledTool>,
}

struct CompiledTool {
    input: schema::Shape,
    output: schema::Shape,
}

impl Catalog {
    fn parse(value: Value, cancelled: &AtomicBool) -> Result<Self, Error> {
        #[derive(Deserialize)]
        struct RawTool {
            name: String,
            #[serde(rename = "inputSchema")]
            input: Value,
            #[serde(rename = "outputSchema", default, deserialize_with = "present_output")]
            output: Option<Value>,
        }
        fn present_output<'de, D: serde::Deserializer<'de>>(
            deserializer: D,
        ) -> Result<Option<Value>, D::Error> {
            Value::deserialize(deserializer).map(Some)
        }
        #[derive(Deserialize)]
        struct RawCatalog {
            tools: Vec<RawTool>,
        }
        let raw: RawCatalog = serde_json::from_value(value)?;
        let mut tools = BTreeMap::new();
        for tool in raw.tools {
            if tool.name.is_empty() || tools.contains_key(&tool.name) {
                return Err(Error::Contract(
                    "tool names must be nonempty and unique".into(),
                ));
            }
            if tool.input.get("type").and_then(Value::as_str) != Some("object") {
                return Err(Error::Contract(format!(
                    "{}: inputSchema must have type object",
                    tool.name
                )));
            }
            let input = schema::Shape::parse(&tool.input, cancelled)?;
            let output = match tool.output {
                None => schema::Shape::default(),
                Some(value) if value.is_object() => schema::Shape::parse(&value, cancelled)?,
                Some(_) => {
                    return Err(Error::Contract(format!(
                        "{}: outputSchema must be an object",
                        tool.name
                    )));
                }
            };
            tools.insert(tool.name, CompiledTool { input, output });
        }
        Ok(Self { tools })
    }
}

pub fn compile(request: Request) -> Result<Generated, Error> {
    compile_cancellable(request, &AtomicBool::new(false))
}

pub fn compile_cancellable(request: Request, cancelled: &AtomicBool) -> Result<Generated, Error> {
    check_cancelled(cancelled)?;
    let contracts = request
        .contracts
        .into_iter()
        .map(|(name, raw)| {
            if name.is_empty() {
                return Err(Error::Contract("binding names must be nonempty".into()));
            }
            Ok((name, Catalog::parse(raw, cancelled)?))
        })
        .collect::<Result<_, Error>>()?;
    let (declarations, bindings) = request.language.generate(&contracts);
    Ok(Generated {
        declarations,
        bindings,
    })
}

pub fn compile_json(input: &str) -> Result<String, Error> {
    Ok(serde_json::to_string(&compile(serde_json::from_str(
        input,
    )?)?)?)
}

fn check_cancelled(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(Ordering::Relaxed) {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}
