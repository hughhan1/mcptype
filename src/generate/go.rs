//! The generated Go package `holon`: declarations and one client variable per dependency.
//! The text is gofmt-clean by construction; `tests/generate.rs` checks it with `gofmt -l`.
use super::{GenerateRequest, declarations};
use crate::{Error, Language};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt::Write;

pub(super) fn generate(request: &GenerateRequest) -> Result<BTreeMap<&'static str, String>, Error> {
    let module = request
        .go_module
        .as_deref()
        .ok_or_else(|| Error::Contract("go_module is required for the go target".into()))?;
    let generated = declarations(Language::Go, &request.contracts)?;
    let bindings = generated.bindings.unwrap_or_default();
    let mut source = String::new();
    for (alias, contract) in &request.contracts {
        let binding = &bindings[alias];
        let name = &binding.name;
        write!(
            source,
            "\nvar {name} = _{name}{{&holon.Dependency{{Name: {}, Tools: map[string]holon.ToolSchema{{",
            quote(alias)
        )
        .unwrap();
        if !contract.tools.is_empty() {
            source.push('\n');
        }
        for (tool, schema) in &contract.tools {
            let output = match &schema.output {
                Some(output) => quote(&serde_json::to_string(output)?),
                None => "\"\"".into(),
            };
            let input = quote(&serde_json::to_string(&Value::Object(
                schema.input.clone(),
            ))?);
            writeln!(
                source,
                "\t{}: {{\n\t\tInput:  {input},\n\t\tOutput: {output},\n\t}},",
                quote(tool)
            )
            .unwrap();
        }
        writeln!(source, "}}}}}}\n\ntype _{name} struct{{ *_dependency }}").unwrap();
        for (tool, method) in &binding.tools {
            writeln!(
                source,
                "\nfunc (client _{name}) {}(ctx context.Context, input {}) ({}, error) {{\n\treturn holon.Call[{}](ctx, client._dependency, {}, input)\n}}",
                method.name,
                method.input,
                method.output,
                method.output,
                quote(tool)
            )
            .unwrap();
        }
    }
    let mut imports = Vec::new();
    if bindings.values().any(|binding| !binding.tools.is_empty()) {
        imports.push("\t\"context\"\n".to_owned());
    }
    if !request.contracts.is_empty() {
        imports.push(format!("\tholon {}\n", quote(module)));
        source.insert_str(0, "\ntype _dependency = holon.Dependency\n");
    }
    let header = match imports.as_slice() {
        [] => String::new(),
        imports => format!("\nimport (\n{})\n", imports.join("\n")),
    };
    let fields = serde_json::to_string(&request.package_fields)?;
    Ok(BTreeMap::from([
        (
            "types.go",
            format!("package holon\n\n{}", generated.declarations),
        ),
        ("clients.go", format!("package holon\n{header}{source}")),
        ("package.json", fields),
    ]))
}

/// A Go interpreted string literal.
fn quote(text: &str) -> String {
    let mut literal = String::from('"');
    for ch in text.chars() {
        match ch {
            '"' => literal += "\\\"",
            '\\' => literal += "\\\\",
            '\n' => literal += "\\n",
            '\r' => literal += "\\r",
            '\t' => literal += "\\t",
            ch if ch.is_control() => literal += &format!("\\u{:04x}", ch as u32),
            ch => literal.push(ch),
        }
    }
    literal.push('"');
    literal
}
