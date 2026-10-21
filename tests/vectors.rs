use mcptype::json::{canonical, changes};
use serde_json::Value;

fn parse(output: String) -> Value {
    serde_json::from_str(&output).unwrap()
}

#[test]
fn shared_vectors_match() {
    let vectors: Value = serde_json::from_str(include_str!("vectors/contract-core.json")).unwrap();
    for vector in vectors["canonical"].as_array().unwrap() {
        let output = parse(canonical(vector["input"].to_string().as_bytes()).unwrap());
        assert_eq!(output, vector["output"], "{}", vector["input"]);
    }
    for vector in vectors["changes"].as_array().unwrap() {
        let output = parse(changes(vector["input"].to_string().as_bytes()).unwrap());
        assert_eq!(output, vector["output"], "{}", vector["input"]);
    }
    for input in vectors["invalid"].as_array().unwrap() {
        assert!(
            changes(input.to_string().as_bytes()).is_err(),
            "accepted {input}"
        );
    }
}
