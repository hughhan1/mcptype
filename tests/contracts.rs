use mcptype::{Language, compile};
use serde_json::{Value, json};

fn compile_contracts(
    language: Language,
    contracts: Value,
) -> Result<mcptype::Generated, mcptype::Error> {
    compile(serde_json::from_value(
        json!({"language": language, "contracts": contracts}),
    )?)
}

#[test]
fn invalid_contracts_are_rejected_before_generation() {
    let tool = json!({"name":"read", "inputSchema":{"type":"object"}});
    for (contracts, message) in [
        (json!({"":{"tools":[]}}), "binding names must be nonempty"),
        (
            json!({"files":{"tools":[tool.clone(), tool.clone()]}}),
            "tool names must be nonempty and unique",
        ),
        (
            json!({"files":{"tools":[{"name":"", "inputSchema":{"type":"object"}}]}}),
            "tool names must be nonempty and unique",
        ),
        (
            json!({"files":{"tools":[{"name":"read", "inputSchema":{"type":"string"}}]}}),
            "read: inputSchema must have type object",
        ),
        (
            json!({"files":{"tools":[{"name":"read", "inputSchema":{"type":"object"}, "outputSchema":null}]}}),
            "invalid type: null, expected a map",
        ),
        (
            json!({"files":{"tools":[{"name":"read", "inputSchema":{"type":"object", "properties":{"x":{"$ref":"https://example.com/schema"}}}}]}}),
            "external schema reference is unsupported",
        ),
        (
            json!({"files":{"tools":[{"name":"read", "inputSchema":{"type":"object", "properties":{"x":{"$ref":"#/$defs/missing"}}}}]}}),
            "unresolved schema reference",
        ),
    ] {
        let error = compile_contracts(Language::TypeScript, contracts).unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
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
    let generated = compile_contracts(
        Language::Go,
        json!({
            "orders":{"tools":[{"name":"get_order","inputSchema":schema,"outputSchema":{"type":"object"}}]},
            "orders!":{"tools":[
                {"name":"get-order","inputSchema":{"type":"object"}},
                {"name":"get_order","inputSchema":{"type":"object"}}
            ]}
        }),
    )
    .unwrap();
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
    if _,ok:=decoded["enabled"];ok{t.Fatal("absent enabled became false")}
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

#[test]
fn declarations_are_plain_modules_without_runtime_clients() {
    let contracts = json!({"files":{"tools":[{"name":"read", "inputSchema":{"type":"object"}}]}});
    let typescript = compile_contracts(Language::TypeScript, contracts.clone())
        .unwrap()
        .declarations;
    assert!(
        typescript.starts_with("export interface Inputs {"),
        "{typescript}"
    );
    assert!(!typescript.contains("mcptype"), "{typescript}");
    let python = compile_contracts(Language::Python, contracts)
        .unwrap()
        .declarations;
    assert!(
        python.contains("class _Clients(_typing.Protocol):"),
        "{python}"
    );
    assert!(
        !python.contains("def mcp(") && !python.contains("from mcp"),
        "{python}"
    );
}
