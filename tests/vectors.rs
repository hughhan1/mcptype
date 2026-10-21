use mcptype::{canonical_json, changes_json};
use serde_json::Value;

#[test]
fn shared_canonical_and_change_vectors_match() {
    let vectors: Value = serde_json::from_str(include_str!("vectors/contract-core.json")).unwrap();
    for vector in vectors["canonical"].as_array().unwrap() {
        let actual: Value =
            serde_json::from_str(&canonical_json(&vector["input"].to_string()).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_string(&actual).unwrap(),
            serde_json::to_string(&vector["output"]).unwrap()
        );
    }
    for vector in vectors["changes"].as_array().unwrap() {
        let actual: Value =
            serde_json::from_str(&changes_json(&vector["input"].to_string()).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_string(&actual).unwrap(),
            serde_json::to_string(&vector["output"]).unwrap()
        );
    }
}

#[test]
fn published_contract_schema_is_valid_json() {
    let schema: Value = serde_json::from_str(mcptype::contract::CONTRACT_SCHEMA).unwrap();
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(schema["properties"]["version"]["const"], 1);
    assert_eq!(
        schema["$defs"]["contract"]["properties"]["tools"]["type"],
        "array"
    );
    assert_eq!(schema["type"], "object");
}
