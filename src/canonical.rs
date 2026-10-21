use crate::{
    Error,
    contract::{Contract, Schema},
};
use ring::digest::{SHA256, digest as sha256};
use serde::Deserialize;
use serde_json::{Value, json};

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
    let mut contract = contract.clone();
    for tool in contract.tools.values_mut() {
        normalize(&mut tool.input.0);
        tool.output = tool.output.take().map(|mut schema| {
            normalize(&mut schema.0);
            schema
        });
    }
    for resource in contract.resources.values_mut() {
        normalize(&mut resource.output.0);
    }
    serde_json::to_value(contract).expect("contracts are serializable")
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
}
