use super::{GenerateRequest, compile_contracts, put};
use crate::{Error, GoBinding, Language};
use std::{collections::BTreeMap, sync::atomic::AtomicBool};

fn render_clients(
    request: &GenerateRequest,
    bindings: &BTreeMap<String, GoBinding>,
) -> Result<String, Error> {
    let module = request
        .go_module
        .as_deref()
        .unwrap_or("github.com/holon-sh/sdk/go");
    let has_typed = request.contracts.keys().any(|alias| {
        bindings
            .get(alias)
            .is_some_and(|binding| !binding.tools.is_empty())
    });
    let has_untyped = request
        .requires
        .keys()
        .any(|alias| !request.contracts.contains_key(alias));
    let mut out = String::from("package holon\n\nimport (\n");
    if has_typed || has_untyped {
        out.push_str("\t\"context\"\n");
    }
    if has_untyped {
        out.push_str("\t\"github.com/modelcontextprotocol/go-sdk/mcp\"\n");
    }
    out.push_str(&format!(
        "\tholon {module:?}\n)\n\ntype _dependency = holon.Dependency\n\n"
    ));
    for (alias, identity) in &request.requires {
        let binding = bindings.get(alias).ok_or_else(|| {
            Error::Contract(format!(
                "missing compiled Go binding for dependency {alias}"
            ))
        })?;
        let name = &binding.name;
        let _ = identity;
        let contract = request.contracts.get(alias);
        out.push_str(&format!("var {name} = _{name}{{&holon.Dependency{{Name:{alias:?},Tools:map[string]holon.ToolSchema{{\n"));
        if let Some(contract) = contract {
            for (tool_name, tool) in &contract.tools {
                let input = serde_json::to_string(&tool.input.0).unwrap_or_default();
                let output = tool
                    .output
                    .as_ref()
                    .map(|value| serde_json::to_string(&value.0).unwrap_or_default())
                    .unwrap_or_default();
                out.push_str(&format!(
                    "{tool_name:?}:{{Input:{input:?},Output:{output:?}}},\n"
                ));
            }
        }
        out.push_str("}}}\n");
        out.push_str(&format!("type _{name} struct {{ *_dependency }}\n"));
        if contract.is_some() {
            for (tool_name, tool) in &binding.tools {
                out.push_str(&format!("func (client _{name}) {}(ctx context.Context, input {}) ({},error) {{ return holon.Call[{}](ctx,client._dependency,{tool_name:?},input) }}\n", tool.name, tool.input, tool.output, tool.output));
            }
        } else {
            out.push_str(&format!("func (client _{name}) CallTool(ctx context.Context, tool string, input any) (*mcp.CallToolResult,error) {{ return holon.CallTool(ctx,client._dependency,tool,input) }}\n"));
        }
    }
    Ok(out)
}

pub(crate) fn generate(
    request: &GenerateRequest,
    cancelled: &AtomicBool,
) -> Result<BTreeMap<String, Vec<u8>>, Error> {
    // Compile empty catalogs for untyped dependencies too. The compiler owns
    // client symbol allocation, so aliases that normalize to the same Go
    // identifier stay unique across typed and untyped clients.
    let mut contracts = request.contracts.clone();
    for alias in request.requires.keys() {
        contracts
            .entry(alias.clone())
            .or_insert_with(|| crate::Contract {
                tools: BTreeMap::new(),
                resources: BTreeMap::new(),
            });
    }
    let compiled = compile_contracts(&contracts, Language::Go, cancelled)?;
    let bindings = compiled.bindings.unwrap_or_default();
    let mut files = BTreeMap::new();
    put(
        &mut files,
        "types.go",
        format!("package holon\n\n{}", compiled.declarations),
    );
    put(
        &mut files,
        "clients.go",
        render_clients(request, &bindings)?,
    );
    let marker = request
        .package_fields
        .get("holon")
        .and_then(|value| value.as_str())
        .unwrap_or("holon-generated-apps-v1");
    put(
        &mut files,
        "package.json",
        format!("{{\"holon\":{}}}", serde_json::to_string(marker)?),
    );
    Ok(files)
}
