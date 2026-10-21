use crate::{
    Error,
    contract::{Contract, Schema},
};
use ring::digest::{SHA256, digest as sha256};
use serde::Deserialize;
use serde_json::{Map, Value, json};

const ANNOTATIONS: &[&str] = &["description", "title", "$comment", "examples"];
const UNORDERED: &[&str] = &["required", "enum", "type", "allOf", "anyOf", "oneOf"];
const SCHEMA_MAPS: &[&str] = &[
    "properties",
    "$defs",
    "definitions",
    "patternProperties",
    "dependentSchemas",
    "dependencies",
];
const SCHEMA_VALUES: &[&str] = &[
    "additionalProperties",
    "unevaluatedProperties",
    "propertyNames",
    "contains",
    "items",
    "prefixItems",
    "additionalItems",
    "unevaluatedItems",
    "not",
    "if",
    "then",
    "else",
    "contentSchema",
];

pub fn canonical(schema: &Schema) -> Value {
    let mut value = schema.0.clone();
    normalize(&mut value);
    value
}

pub fn digest(contract: &Contract) -> String {
    digest_value(&contract_value(contract))
}

pub fn canonical_contract(contract: &Contract) -> Value {
    contract_value(contract)
}

pub fn canonical_json(input: &str) -> Result<String, Error> {
    #[derive(Deserialize)]
    struct Request {
        #[serde(default)]
        contract: Option<Contract>,
        #[serde(default)]
        schema: Option<Schema>,
    }

    let request: Request = serde_json::from_str(input)?;
    match (request.contract, request.schema) {
        (Some(contract), None) => {
            contract.validate().map_err(Error::Contract)?;
            Ok(serde_json::to_string(&json!({
                "canonical": contract_value(&contract),
                "digest": digest(&contract)
            }))?)
        }
        (None, Some(schema)) if schema.0.is_object() => {
            let value = canonical(&schema);
            let schema_digest = digest_value(&value);
            Ok(serde_json::to_string(&json!({
                "canonical": value,
                "digest": schema_digest
            }))?)
        }
        (None, Some(_)) => Err(Error::Contract("schema must be a JSON object".into())),
        (None, None) => Err(Error::Contract("expected a contract or schema".into())),
        (Some(_), Some(_)) => Err(Error::Contract(
            "provide a contract or schema, not both".into(),
        )),
    }
}

fn contract_value(contract: &Contract) -> Value {
    let tools = contract
        .tools
        .iter()
        .map(|(name, tool)| {
            let mut fields = Map::new();
            fields.insert("name".into(), Value::String(name.clone()));
            fields.insert("inputSchema".into(), canonical(&tool.input));
            if let Some(output) = &tool.output {
                fields.insert("outputSchema".into(), canonical(output));
            }
            Value::Object(fields)
        })
        .collect::<Vec<_>>();
    let resources = contract
        .resources
        .iter()
        .map(|(uri, resource)| json!({"uri": uri, "outputSchema": canonical(&resource.output)}))
        .collect::<Vec<_>>();
    let mut fields = Map::new();
    fields.insert("tools".into(), Value::Array(tools));
    if !resources.is_empty() {
        fields.insert("resources".into(), Value::Array(resources));
    }
    Value::Object(fields)
}

fn digest_value(value: &Value) -> String {
    let bytes = serde_json::to_vec(value).expect("JSON values are serializable");
    let hash = sha256(&SHA256, &bytes);
    let hex = hash
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

fn normalize(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.retain(|key, _| !ANNOTATIONS.contains(&key.as_str()));
            for (key, item) in object.iter_mut() {
                if UNORDERED.contains(&key.as_str()) {
                    if let Value::Array(items) = item {
                        if key.as_str() != "enum" {
                            for item in items.iter_mut() {
                                normalize(item);
                            }
                        }
                        items.sort_by_cached_key(|item| {
                            serde_json::to_vec(item).expect("JSON values are serializable")
                        });
                    }
                } else if SCHEMA_MAPS.contains(&key.as_str()) {
                    if let Value::Object(schemas) = item {
                        for schema in schemas.values_mut() {
                            normalize(schema);
                        }
                    }
                } else if SCHEMA_VALUES.contains(&key.as_str()) {
                    normalize(item);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                normalize(item);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn canonical_schema_drops_annotations_sorts_unordered_keywords_and_recurses() {
        let input = Schema(json!({
            "type":"object", "description":"ignored", "required":["z","a"],
            "properties": {
                "x":{"type":["string","null"], "title":"ignored", "enum":["z","a"]},
                "y":{"not":{"type":"string", "$comment":"ignored"}}
            },
            "dependentSchemas":{"description":{"required":["b","a"]}}
        }));
        assert_eq!(
            canonical(&input),
            json!({
                "type":"object", "required":["a","z"],
                "properties": {
                    "x":{"type":["null","string"], "enum":["a","z"]},
                    "y":{"not":{"type":"string"}}
                },
                "dependentSchemas":{"description":{"required":["a","b"]}}
            })
        );
    }

    #[test]
    fn digest_ignores_documentation_and_wire_order() {
        let left: Contract = serde_json::from_value(json!({
            "tools":[
                {"name":"z","inputSchema":{"type":"object"}},
                {"name":"a","inputSchema":{"type":"object","title":"before"}}
            ]
        }))
        .unwrap();
        let right: Contract = serde_json::from_value(json!({
            "tools":[
                {"name":"a","inputSchema":{"title":"after","type":"object"}},
                {"name":"z","inputSchema":{"type":"object"}}
            ]
        }))
        .unwrap();
        assert_eq!(digest(&left), digest(&right));
    }

    #[test]
    fn annotations_inside_dependency_names_are_not_dropped() {
        let left =
            Schema(json!({"type":"object","dependentSchemas":{"description":{"required":["a"]}}}));
        let right =
            Schema(json!({"type":"object","dependentSchemas":{"description":{"required":["b"]}}}));
        assert_ne!(canonical(&left), canonical(&right));
    }

    #[test]
    fn contract_digest_handles_resources() {
        let contract = Contract {
            tools: BTreeMap::new(),
            resources: BTreeMap::from([(
                "orders://summary".into(),
                crate::contract::Resource {
                    output: Schema(
                        json!({"type":"object","properties":{"total":{"type":"number","description":"total"}}}),
                    ),
                },
            )]),
        };
        assert!(digest(&contract).starts_with("sha256:"));
    }
}
