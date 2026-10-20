use crate::{
    Catalog, quoted,
    schema::{Extra, Type},
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Serialize)]
pub struct Binding {
    pub name: String,
    pub tools: BTreeMap<String, Tool>,
}

#[derive(Debug, Serialize)]
pub struct Tool {
    pub name: String,
    pub input: String,
    pub output: String,
}

fn identifier(value: &str) -> String {
    let mut result = String::new();
    let mut upper = true;
    for ch in value.chars() {
        if !ch.is_ascii_alphanumeric() {
            upper = true;
            continue;
        }
        if result.is_empty() && ch.is_ascii_digit() {
            result.push('X');
        }
        result.push(if upper { ch.to_ascii_uppercase() } else { ch });
        upper = false;
    }
    if result.is_empty() {
        result.push_str("Value");
    }
    result
}

fn unique(names: &mut BTreeSet<String>, preferred: String) -> String {
    let mut name = preferred.clone();
    let mut suffix = 2;
    while !names.insert(name.clone()) {
        name = format!("{preferred}{suffix}");
        suffix += 1;
    }
    name
}

fn field_name(value: &str) -> bool {
    !value.is_empty()
        && value != "-"
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || " !#$%&()*+-./:;<=>?@[]^_{|}~".contains(ch))
}

fn render(value: &Type) -> String {
    match value {
        Type::String => "string".into(),
        Type::Integer | Type::Number => "json.Number".into(),
        Type::Boolean => "bool".into(),
        Type::Literal(value) if value.is_string() => "string".into(),
        Type::Literal(value) if value.is_boolean() => "bool".into(),
        Type::Literal(value) if value.is_number() => "json.Number".into(),
        Type::Array(item) => format!("[]{}", render(item)),
        Type::Object { fields, extra } if fields.is_empty() => format!(
            "map[string]{}",
            match extra {
                Extra::Open(value) => render(value),
                _ => "json.RawMessage".into(),
            }
        ),
        Type::Object {
            fields,
            extra: Extra::Closed,
        } if fields.keys().all(|name| field_name(name)) => {
            let mut names = BTreeSet::new();
            let mut output = String::from("struct {\n");
            for (name, field) in fields {
                let mut value = render(&field.value);
                if !field.required && value != "json.RawMessage" {
                    value = format!("*{value}");
                }
                let tag = format!(
                    "json:{}",
                    quoted(&format!(
                        "{name}{}",
                        if field.required { "" } else { ",omitempty" }
                    ))
                );
                output.push_str(&format!(
                    "\t{} {value} {}\n",
                    unique(&mut names, identifier(name)),
                    quoted(&tag)
                ));
            }
            output + "}"
        }
        Type::Object { .. } => "map[string]json.RawMessage".into(),
        _ => "json.RawMessage".into(),
    }
}

pub(crate) fn generate(
    contracts: &BTreeMap<String, Catalog>,
) -> (String, BTreeMap<String, Binding>) {
    let mut names = BTreeSet::new();
    let mut declarations = String::new();
    let mut bindings = BTreeMap::new();
    for (binding, catalog) in contracts {
        let client = unique(&mut names, identifier(binding));
        let mut methods = BTreeSet::from(["CallTool".into()]);
        let mut tools = BTreeMap::new();
        for (name, tool) in &catalog.tools {
            let method = unique(&mut methods, identifier(name));
            let mut shapes = Vec::new();
            for (suffix, shape) in [("Input", &tool.input), ("Output", &tool.output)] {
                let symbol = unique(&mut names, format!("{client}{method}{suffix}"));
                declarations.push_str(&format!("type {symbol} = {}\n\n", render(&shape.value)));
                for (definition, value) in &shape.named {
                    let symbol = unique(&mut names, format!("{symbol}{}", identifier(definition)));
                    declarations.push_str(&format!("type {symbol} = {}\n\n", render(value)));
                }
                shapes.push(symbol);
            }
            tools.insert(
                name.clone(),
                Tool {
                    name: method,
                    input: shapes[0].clone(),
                    output: shapes[1].clone(),
                },
            );
        }
        bindings.insert(
            binding.clone(),
            Binding {
                name: client,
                tools,
            },
        );
    }
    if declarations.contains("json.") {
        declarations.insert_str(0, "import \"encoding/json\"\n\n");
    }
    (declarations, bindings)
}
