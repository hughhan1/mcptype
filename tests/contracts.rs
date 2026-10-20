use mcptype::{Catalog, Language, Request, compile};
use serde_json::json;
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
fn go_types_preserve_json_and_compile() {
    let schema = json!({"type":"object","additionalProperties":false,"properties":{
        "name":{"type":"string"},
        "count":{"type":"integer"},
        "enabled":{"type":"boolean"},
        "nullable":{"anyOf":[{"type":"string"},{"type":"null"}]},
        "labels":{"type":"object","additionalProperties":{"type":"string"}},
        "open":{"type":"object","properties":{"known":{"type":"string"}}},
        "item":{"$ref":"#/$defs/Item"}
    },"required":["name","count","labels","open","item"],"$defs":{
        "Item":{"type":"object","additionalProperties":false,"properties":{"id":{"type":"string"}},"required":["id"]}
    }});
    let mut contracts = BTreeMap::from([
        (
            "orders".into(),
            json!({"tools":[{"name":"get_order","inputSchema":schema,"outputSchema":{"type":"object"}}]}),
        ),
        (
            "orders!".into(),
            json!({"tools":[{"name":"get-order","inputSchema":{"type":"object"}},{"name":"get_order","inputSchema":{"type":"object"}}]}),
        ),
    ]);
    let generated = compile(Request {
        language: Language::Go,
        contracts: contracts.clone(),
    })
    .unwrap();
    for catalog in contracts.values_mut() {
        catalog["tools"].as_array_mut().unwrap().reverse();
    }
    let reordered = compile(Request {
        language: Language::Go,
        contracts,
    })
    .unwrap();
    assert_eq!(
        serde_json::to_value(&generated).unwrap(),
        serde_json::to_value(reordered).unwrap()
    );
    let bindings = generated.bindings.as_ref().unwrap();
    assert_ne!(bindings["orders"].name, bindings["orders!"].name);
    assert_ne!(
        bindings["orders!"].tools["get-order"].name,
        bindings["orders!"].tools["get_order"].name
    );
    let directory = std::env::temp_dir().join(format!("mcptype-go-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("go.mod"), "module contract\n\ngo 1.27.0\n").unwrap();
    std::fs::write(
        directory.join("types.go"),
        format!("package contract\n\n{}", generated.declarations),
    )
    .unwrap();
    std::fs::write(directory.join("types_test.go"), r#"package contract
import ("encoding/json"; "testing")
func TestContract(t *testing.T) {
    var value OrdersGetOrderInput
    raw := []byte(`{"name":"one","count":9007199254740993,"enabled":false,"nullable":null,"labels":{"team":"blue"},"open":{"known":"a","extra":7},"item":{"id":"x"}}`)
    if err := json.Unmarshal(raw,&value); err != nil { t.Fatal(err) }
    if value.Name != "one" || value.Count.String() != "9007199254740993" || value.Enabled == nil || *value.Enabled || string(value.Nullable) != "null" || value.Labels["team"] != "blue" || string(value.Open["extra"]) != "7" || value.Item.Id != "x" { t.Fatalf("lost data: %+v",value) }
    roundtrip,err:=json.Marshal(value); if err!=nil {t.Fatal(err)}
    var decoded map[string]json.RawMessage
    if err:=json.Unmarshal(roundtrip,&decoded);err!=nil{t.Fatal(err)}
    if string(decoded["nullable"])!="null" || string(decoded["enabled"])!="false" {t.Fatalf("lost presence: %s",roundtrip)}
    var absent OrdersGetOrderInput
    roundtrip,err=json.Marshal(absent);if err!=nil{t.Fatal(err)}
    decoded=nil
    if err:=json.Unmarshal(roundtrip,&decoded);err!=nil{t.Fatal(err)}
    if _,ok:=decoded["nullable"];ok{t.Fatal("absent nullable became null")}
    if _,ok:=decoded["enabled"];ok{t.Fatal("absent bool became false")}
}
"#).unwrap();
    let result = std::process::Command::new("go")
        .args(["test", "./..."])
        .env("GOWORK", "off")
        .current_dir(&directory)
        .output()
        .expect("Go is required for generated contract checks");
    std::fs::remove_dir_all(directory).unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
