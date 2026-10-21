use crate::contract::{Contract, Resource, Schema, Tool};
use serde_json::Value;
use sha2::{Digest, Sha256};

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

/// Drops annotations and sorts order-insensitive keywords, recursively.
pub fn canonical(schema: &Schema) -> Schema {
    let mut schema = schema.clone();
    normalize_object(&mut schema);
    schema
}

/// The contract with every schema in canonical form.
pub fn canonical_contract(contract: &Contract) -> Contract {
    Contract {
        tools: contract
            .tools
            .iter()
            .map(|(name, tool)| {
                let tool = Tool {
                    input: canonical(&tool.input),
                    output: tool.output.as_ref().map(canonical),
                };
                (name.clone(), tool)
            })
            .collect(),
        resources: contract
            .resources
            .iter()
            .map(|(uri, resource)| {
                (
                    uri.clone(),
                    Resource {
                        output: canonical(&resource.output),
                    },
                )
            })
            .collect(),
    }
}

/// `sha256:<hex>` of the canonical contract's JSON with sorted keys and no whitespace.
pub fn digest(contract: &Contract) -> String {
    hash(&serde_json::to_value(canonical_contract(contract)).expect("contracts serialize"))
}

pub(crate) fn hash(value: &Value) -> String {
    let hex: String = Sha256::digest(encode(value))
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("sha256:{hex}")
}

/// Compact JSON with object keys sorted, independent of serde_json's map ordering feature.
fn encode(value: &Value) -> Vec<u8> {
    fn write(value: &Value, out: &mut Vec<u8>) {
        match value {
            Value::Object(object) => {
                let mut entries: Vec<_> = object.iter().collect();
                entries.sort_unstable_by_key(|(key, _)| *key);
                out.push(b'{');
                for (index, (key, item)) in entries.into_iter().enumerate() {
                    if index > 0 {
                        out.push(b',');
                    }
                    serde_json::to_writer(&mut *out, key).expect("strings serialize");
                    out.push(b':');
                    write(item, out);
                }
                out.push(b'}');
            }
            Value::Array(items) => {
                out.push(b'[');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push(b',');
                    }
                    write(item, out);
                }
                out.push(b']');
            }
            scalar => serde_json::to_writer(&mut *out, scalar).expect("scalars serialize"),
        }
    }
    let mut out = Vec::new();
    write(value, &mut out);
    out
}

fn normalize(value: &mut Value) {
    match value {
        Value::Object(object) => normalize_object(object),
        Value::Array(items) => items.iter_mut().for_each(normalize),
        _ => {}
    }
}

fn normalize_object(object: &mut Schema) {
    object.retain(|key, _| !ANNOTATIONS.contains(&key.as_str()));
    for (key, item) in object.iter_mut() {
        let key = key.as_str();
        if UNORDERED.contains(&key) {
            if let Value::Array(items) = item {
                if key != "enum" {
                    items.iter_mut().for_each(normalize);
                }
                items.sort_by_cached_key(encode);
            }
        } else if SCHEMA_MAPS.contains(&key) {
            if let Value::Object(schemas) = item {
                schemas.values_mut().for_each(normalize);
            }
        } else if SCHEMA_VALUES.contains(&key) {
            normalize(item);
        }
    }
}
