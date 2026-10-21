pub mod canonical;
pub mod changes;
pub mod check;
pub mod contract;
pub mod generate;
mod go;
pub mod json;
mod python;
mod schema;
mod typescript;

pub use canonical::{canonical, canonical_contract, digest};
pub use changes::{Change, changes};
pub use contract::{Contract, Expectation, Identity, Metadata, Resource, Schema, Tool};
pub use generate::{GenerateRequest, GeneratedPackage, Target, generate, generate_json};
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
    pub contracts: BTreeMap<String, Contract>,
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
    fn parse(contract: Contract, cancelled: &AtomicBool) -> Result<Self, Error> {
        let mut tools = BTreeMap::new();
        for (name, tool) in contract.tools {
            check_cancelled(cancelled)?;
            let input = schema::Shape::parse(&Value::Object(tool.input), cancelled)?;
            let output = match tool.output {
                Some(output) => schema::Shape::parse(&Value::Object(output), cancelled)?,
                None => schema::Shape::default(),
            };
            tools.insert(name, CompiledTool { input, output });
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
    check_cancelled(cancelled)?;
    Ok(Generated {
        declarations,
        bindings,
    })
}

fn check_cancelled(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(Ordering::Relaxed) {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}
