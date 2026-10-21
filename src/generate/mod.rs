//! Typed client packages: the files one target needs, laid out from a contract set.
mod go;
mod python;
mod typescript;

use crate::{Contract, Error, Generated, Identity, Language, Request};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    Node,
    Browser,
    Python,
    Go,
}

/// One target's package. Over JSON, `runtime` values are base64.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerateRequest {
    pub target: Target,
    pub identity: Identity,
    /// Every declared dependency alias, whether or not it has a contract.
    #[serde(default)]
    pub requires: BTreeMap<String, Identity>,
    /// The typed dependencies for this target, by alias.
    #[serde(default)]
    pub contracts: BTreeMap<String, Contract>,
    /// Files copied into the package verbatim.
    #[serde(default, deserialize_with = "decode")]
    pub runtime: BTreeMap<String, Vec<u8>>,
    /// Node and browser: protocol declarations spliced into each `.d.ts` entry point.
    #[serde(default)]
    pub protocol: Option<String>,
    /// Extra `package.json` fields.
    #[serde(default)]
    pub package_fields: Map<String, Value>,
    /// Go: import path of the runtime module.
    #[serde(default)]
    pub go_module: Option<String>,
}

/// Package files by relative path. Over JSON, values are base64.
#[derive(Debug, Serialize)]
pub struct GeneratedPackage {
    #[serde(serialize_with = "encode")]
    pub files: BTreeMap<String, Vec<u8>>,
}

/// The identity a generated client announces, with the aliases its app declares.
#[derive(Serialize)]
struct Caller<'a> {
    name: &'a str,
    requires: &'a BTreeMap<String, Identity>,
    version: &'a str,
}

pub fn generate(request: GenerateRequest) -> Result<GeneratedPackage, Error> {
    if let Some(path) = request.runtime.keys().find(|path| !relative(path)) {
        return Err(Error::Contract(format!(
            "runtime path {path:?} must be relative and normalized"
        )));
    }
    let caller = Caller {
        name: &request.identity.name,
        version: &request.identity.version,
        requires: &request.requires,
    };
    let generated = match request.target {
        Target::Node | Target::Browser => typescript::generate(&request, &caller)?,
        Target::Python => python::generate(&request, &caller)?,
        Target::Go => go::generate(&request)?,
    };
    let mut files = request.runtime;
    files.extend(
        generated
            .into_iter()
            .map(|(path, text)| (path.to_owned(), text.into_bytes())),
    );
    Ok(GeneratedPackage { files })
}

impl GenerateRequest {
    /// Declared aliases without a contract: their clients call tools by name, unvalidated.
    fn untyped(&self) -> impl Iterator<Item = &String> {
        self.requires
            .keys()
            .filter(|alias| !self.contracts.contains_key(*alias))
    }
}

fn relative(path: &str) -> bool {
    !path.starts_with('/') && path.split('/').all(|part| !matches!(part, "" | "." | ".."))
}

fn declarations(
    language: Language,
    contracts: &BTreeMap<String, Contract>,
) -> Result<Generated, Error> {
    crate::compile(Request {
        language,
        contracts: contracts.clone(),
    })
}

fn decode<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, Vec<u8>>, D::Error> {
    BTreeMap::<String, String>::deserialize(deserializer)?
        .into_iter()
        .map(|(path, data)| {
            let bytes = STANDARD
                .decode(data)
                .map_err(|error| D::Error::custom(format!("runtime {path}: {error}")))?;
            Ok((path, bytes))
        })
        .collect()
}

fn encode<S: Serializer>(
    files: &BTreeMap<String, Vec<u8>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_map(
        files
            .iter()
            .map(|(path, data)| (path, STANDARD.encode(data))),
    )
}
