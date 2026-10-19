use mcptype::{Catalog, Language, Request, compile};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[test]
fn invalid_catalogs_are_rejected_before_generation() {
    let tool = json!({"name":"read", "inputSchema":{"type":"object"}});
    for value in [
        json!({"tools":[tool.clone(),tool.clone()]}),
        json!({"tools":[{"name":"", "inputSchema":{"type":"object"}}]}),
        json!({"tools":[{"name":"read", "inputSchema":{"type":"string"}}]}),
        json!({"tools":[{"name":"read", "inputSchema":{"type":"object"}, "outputSchema":null}]}),
        json!({"tools":[{"name":"read", "inputSchema":{"type":"object", "properties":{"x":{"$ref":"https://example.com/schema"}}}}]}),
        json!({"tools":[{"name":"read", "inputSchema":{"type":"object", "properties":{"x":{"$ref":"#/$defs/missing"}}}}]}),
    ] {
        assert!(Catalog::try_from(value).is_err());
    }
}

#[test]
fn tool_order_does_not_change_declarations() {
    let tools = json!([
        {"name":"read", "inputSchema":{"type":"object"}},
        {"name":"write", "inputSchema":{"type":"object"}}
    ]);
    let reversed = Value::Array(tools.as_array().unwrap().iter().rev().cloned().collect());
    for language in [Language::TypeScript, Language::Python] {
        let emit = |tools: Value| {
            compile(Request {
                language,
                contracts: BTreeMap::from([("files".into(), json!({"tools":tools}))]),
            })
            .unwrap()
            .declarations
        };
        assert_eq!(emit(tools.clone()), emit(reversed.clone()));
    }
}
