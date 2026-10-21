use serde::{Deserialize, Serialize, de::Error as DeError, ser::SerializeStruct};
use serde_json::Value;
use std::collections::BTreeMap;

pub const CONTRACT_SCHEMA: &str = include_str!("../schema/contract.schema.json");

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Identity {
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Schema(pub Value);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "inputSchema")]
    pub input: Schema,
    #[serde(rename = "outputSchema", skip_serializing_if = "Option::is_none")]
    pub output: Option<Schema>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Resource {
    #[serde(rename = "outputSchema")]
    pub output: Schema,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Contract {
    pub tools: BTreeMap<String, Tool>,
    pub resources: BTreeMap<String, Resource>,
}

#[derive(Deserialize)]
struct WireTool {
    name: String,
    #[serde(flatten)]
    tool: Tool,
}

#[derive(Deserialize)]
struct WireResource {
    uri: String,
    #[serde(flatten)]
    resource: Resource,
}

#[derive(Deserialize)]
struct WireContract {
    tools: Vec<WireTool>,
    #[serde(default)]
    resources: Vec<WireResource>,
}

impl<'de> Deserialize<'de> for Contract {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = WireContract::deserialize(deserializer)?;
        let mut tools = BTreeMap::new();
        for item in wire.tools {
            if tools.insert(item.name, item.tool).is_some() {
                return Err(D::Error::custom("duplicate tool name"));
            }
        }
        let mut resources = BTreeMap::new();
        for item in wire.resources {
            if resources.insert(item.uri, item.resource).is_some() {
                return Err(D::Error::custom("duplicate resource URI"));
            }
        }
        Ok(Self { tools, resources })
    }
}

impl Serialize for Contract {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct ToolRef<'a> {
            name: &'a str,
            #[serde(flatten)]
            tool: &'a Tool,
        }
        #[derive(Serialize)]
        struct ResourceRef<'a> {
            uri: &'a str,
            #[serde(flatten)]
            resource: &'a Resource,
        }

        let tools = self
            .tools
            .iter()
            .map(|(name, tool)| ToolRef { name, tool })
            .collect::<Vec<_>>();
        let resources = self
            .resources
            .iter()
            .map(|(uri, resource)| ResourceRef { uri, resource })
            .collect::<Vec<_>>();
        let mut state = serializer.serialize_struct("Contract", 2)?;
        state.serialize_field("tools", &tools)?;
        if !resources.is_empty() {
            state.serialize_field("resources", &resources)?;
        }
        state.end()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Expectation {
    pub identity: Identity,
    pub contract: Contract,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Metadata {
    pub version: u32,
    pub identity: Identity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract: Option<Contract>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub requires: BTreeMap<String, Expectation>,
}

impl Identity {
    pub fn is_valid(&self) -> bool {
        !self.name.is_empty() && !self.version.is_empty()
    }
}

impl Contract {
    pub fn validate(&self) -> Result<(), String> {
        for (name, tool) in &self.tools {
            if name.is_empty() || !is_object_schema(&tool.input.0) {
                return Err("invalid MCP tool contract".into());
            }
            if tool
                .output
                .as_ref()
                .is_some_and(|schema| !schema.0.is_object())
            {
                return Err("invalid MCP tool contract".into());
            }
        }
        for (uri, resource) in &self.resources {
            if uri.is_empty() || !resource.output.0.is_object() {
                return Err("invalid MCP resource contract".into());
            }
        }
        Ok(())
    }
}

impl Expectation {
    pub fn validate(&self) -> Result<(), String> {
        if !self.identity.is_valid() {
            return Err("invalid MCP expectation identity".into());
        }
        self.contract.validate()
    }
}

impl Metadata {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 || !self.identity.is_valid() {
            return Err("invalid Holon MCP contract metadata version or identity".into());
        }
        if let Some(contract) = &self.contract {
            contract.validate()?;
        }
        for (alias, expected) in &self.requires {
            if alias.is_empty() {
                return Err("invalid Holon MCP dependency identity".into());
            }
            expected.validate()?;
        }
        Ok(())
    }
}

fn is_object_schema(value: &Value) -> bool {
    value.get("type").and_then(Value::as_str) == Some("object")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn contract_wire_round_trip_is_sorted_and_preserves_v1_arrays() {
        let contract: Contract = serde_json::from_value(json!({
            "tools": [
                {"name":"z", "inputSchema":{"type":"object"}},
                {"name":"a", "inputSchema":{"type":"object"}}
            ],
            "resources": [
                {"uri":"z://r", "outputSchema":{"type":"object"}},
                {"uri":"a://r", "outputSchema":{"type":"object"}}
            ]
        }))
        .unwrap();
        assert_eq!(
            serde_json::to_value(&contract).unwrap(),
            json!({
                "tools": [
                    {"name":"a", "inputSchema":{"type":"object"}},
                    {"name":"z", "inputSchema":{"type":"object"}}
                ],
                "resources": [
                    {"uri":"a://r", "outputSchema":{"type":"object"}},
                    {"uri":"z://r", "outputSchema":{"type":"object"}}
                ]
            })
        );
    }

    #[test]
    fn duplicate_wire_names_are_rejected() {
        let duplicate_tool = json!({"tools":[
            {"name":"read", "inputSchema":{"type":"object"}},
            {"name":"read", "inputSchema":{"type":"object"}}
        ]});
        let duplicate_resource = json!({"tools":[], "resources":[
            {"uri":"orders://r", "outputSchema":{"type":"object"}},
            {"uri":"orders://r", "outputSchema":{"type":"object"}}
        ]});
        assert!(serde_json::from_value::<Contract>(duplicate_tool).is_err());
        assert!(serde_json::from_value::<Contract>(duplicate_resource).is_err());
    }
}
