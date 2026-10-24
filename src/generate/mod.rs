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
    /// Node: import the installed runtime instead of a copied runtime file.
    #[serde(default)]
    pub runtime_specifier: Option<String>,
    /// Browser: import the installed runtime instead of a copied runtime file.
    #[serde(default)]
    pub browser_runtime_specifier: Option<String>,
    /// Import protocol declarations instead of inlining `protocol`.
    #[serde(default)]
    pub protocol_specifier: Option<String>,
    /// Python: normalized relative output path instead of `_apps.py`.
    #[serde(default)]
    pub python_module: Option<String>,
    /// Python: absolute runtime import module instead of `.client`.
    #[serde(default)]
    pub python_runtime_module: Option<String>,
    /// First-line comment on generated source files (not JSON or copied runtime files).
    #[serde(default)]
    pub banner: Option<String>,
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
    request.validate()?;
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
    let mut files = request.runtime.clone();
    for (path, mut text) in generated {
        if let Some(banner) = &request.banner
            && path != "package.json"
        {
            let comment = if path.ends_with(".py") { "#" } else { "//" };
            text.insert_str(0, &format!("{comment} {banner}\n"));
        }
        files.insert(path.to_owned(), text.into_bytes());
    }
    Ok(GeneratedPackage { files })
}

impl GenerateRequest {
    fn validate(&self) -> Result<(), Error> {
        if self.protocol.is_some() && self.protocol_specifier.is_some() {
            return Err(Error::Contract(
                "protocol and protocol_specifier are mutually exclusive".into(),
            ));
        }
        for (name, value) in [
            ("runtime_specifier", &self.runtime_specifier),
            ("browser_runtime_specifier", &self.browser_runtime_specifier),
            ("protocol_specifier", &self.protocol_specifier),
            ("banner", &self.banner),
        ] {
            if let Some(value) = value
                && (value.trim().is_empty()
                    || value
                        .chars()
                        .any(|ch| ch.is_control() || matches!(ch, '\u{2028}' | '\u{2029}')))
            {
                return Err(Error::Contract(format!(
                    "{name} must be nonempty and contain no line breaks or control characters"
                )));
            }
        }
        if let Some(path) = &self.python_module
            && (!relative(path)
                || path.contains(['\\', ':'])
                || path.chars().any(char::is_control)
                || !path.ends_with(".py"))
        {
            return Err(Error::Contract(
                "python_module must be a relative, normalized .py path".into(),
            ));
        }
        if let Some(module) = &self.python_runtime_module
            && !module.split('.').all(|part| {
                crate::python::identifier(part)
                    || (part.starts_with('_')
                        && part
                            .chars()
                            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_'))
            })
        {
            return Err(Error::Contract(
                "python_runtime_module must be an absolute Python module name".into(),
            ));
        }
        Ok(())
    }

    fn slot(&self) -> bool {
        self.runtime_specifier.is_some()
            || self.browser_runtime_specifier.is_some()
            || self.python_module.is_some()
    }

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
