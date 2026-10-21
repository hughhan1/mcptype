//! The v1 contract document. Deserializing a value validates it; there is no separate `validate` step.
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// A JSON Schema. Only JSON objects deserialize.
pub type Schema = Map<String, Value>;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Identity {
    #[serde(deserialize_with = "non_empty")]
    pub name: String,
    #[serde(deserialize_with = "non_empty")]
    pub version: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tool {
    pub input: Schema,
    pub output: Option<Schema>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Resource {
    pub output: Schema,
}

/// Tools keyed by name and resources keyed by URI. The wire format is v1's arrays, written in key order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Wire", into = "Wire")]
pub struct Contract {
    pub tools: BTreeMap<String, Tool>,
    pub resources: BTreeMap<String, Resource>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Expectation {
    pub identity: Identity,
    pub contract: Contract,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Metadata {
    #[serde(deserialize_with = "version")]
    pub version: u32,
    pub identity: Identity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract: Option<Contract>,
    #[serde(
        default,
        deserialize_with = "aliases",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub requires: BTreeMap<String, Expectation>,
}

#[derive(Serialize, Deserialize)]
struct Wire {
    tools: Vec<WireTool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    resources: Vec<WireResource>,
}

#[derive(Serialize, Deserialize)]
struct WireTool {
    name: String,
    #[serde(rename = "inputSchema")]
    input: Schema,
    #[serde(
        rename = "outputSchema",
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    output: Option<Schema>,
}

#[derive(Serialize, Deserialize)]
struct WireResource {
    uri: String,
    #[serde(rename = "outputSchema")]
    output: Schema,
}

impl TryFrom<Wire> for Contract {
    type Error = String;

    fn try_from(wire: Wire) -> Result<Self, String> {
        let mut contract = Self::default();
        for WireTool {
            name,
            input,
            output,
        } in wire.tools
        {
            if input.get("type") != Some(&Value::from("object")) {
                return Err(format!("{name}: inputSchema must have type object"));
            }
            if name.is_empty()
                || contract
                    .tools
                    .insert(name, Tool { input, output })
                    .is_some()
            {
                return Err("tool names must be nonempty and unique".into());
            }
        }
        for WireResource { uri, output } in wire.resources {
            if uri.is_empty()
                || contract
                    .resources
                    .insert(uri, Resource { output })
                    .is_some()
            {
                return Err("resource URIs must be nonempty and unique".into());
            }
        }
        Ok(contract)
    }
}

impl From<Contract> for Wire {
    fn from(contract: Contract) -> Self {
        Self {
            tools: contract
                .tools
                .into_iter()
                .map(|(name, Tool { input, output })| WireTool {
                    name,
                    input,
                    output,
                })
                .collect(),
            resources: contract
                .resources
                .into_iter()
                .map(|(uri, Resource { output })| WireResource { uri, output })
                .collect(),
        }
    }
}

/// Rejects `"outputSchema": null`, which `Option` would otherwise read as absent.
fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Schema>, D::Error> {
    Schema::deserialize(deserializer).map(Some)
}

fn non_empty<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let value = String::deserialize(deserializer)?;
    if value.is_empty() {
        return Err(D::Error::custom(
            "identity name and version must be nonempty",
        ));
    }
    Ok(value)
}

fn version<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    match u32::deserialize(deserializer)? {
        1 => Ok(1),
        version => Err(D::Error::custom(format!(
            "unsupported metadata version {version}"
        ))),
    }
}

fn aliases<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, Expectation>, D::Error> {
    let requires = BTreeMap::<String, Expectation>::deserialize(deserializer)?;
    if requires.contains_key("") {
        return Err(D::Error::custom("dependency aliases must be nonempty"));
    }
    Ok(requires)
}
