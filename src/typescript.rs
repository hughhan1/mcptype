use crate::{
    Catalog, quoted,
    schema::{Extra, Type},
};
use std::collections::BTreeMap;

fn render(value: &Type) -> String {
    match value {
        Type::Unknown => "unknown".into(),
        Type::Never => "never".into(),
        Type::String => "string".into(),
        Type::Integer | Type::Number => "number".into(),
        Type::Boolean => "boolean".into(),
        Type::Null => "null".into(),
        Type::Literal(value) if value.is_array() || value.is_object() => "unknown".into(),
        Type::Literal(value) => value.to_string(),
        Type::Array(item) => format!("Array<{}>", render(item)),
        Type::Union(items) => {
            if items.is_empty() {
                "never".into()
            } else {
                format!(
                    "({})",
                    items.iter().map(render).collect::<Vec<_>>().join(" | ")
                )
            }
        }
        Type::Intersection(items) => {
            if items.is_empty() {
                "unknown".into()
            } else {
                format!(
                    "({})",
                    items.iter().map(render).collect::<Vec<_>>().join(" & ")
                )
            }
        }
        Type::Tuple {
            prefix,
            required,
            rest,
        } => {
            let mut items = prefix
                .iter()
                .enumerate()
                .map(|(i, v)| format!("{}{}", render(v), if i >= *required { "?" } else { "" }))
                .collect::<Vec<_>>();
            match rest {
                Extra::Closed => (),
                Extra::Implicit => items.push("...unknown[]".into()),
                Extra::Open(value) => items.push(format!("...Array<{}>", render(value))),
            }
            format!("[{}]", items.join(", "))
        }
        Type::Object { fields, extra } => {
            let mut entries = fields
                .iter()
                .map(|(name, field)| {
                    format!(
                        "{}{}: {};",
                        quoted(name),
                        if field.required { "" } else { "?" },
                        render(&field.value)
                    )
                })
                .collect::<Vec<_>>();
            match extra {
                Extra::Closed => (),
                Extra::Implicit => entries.push("[key: string]: unknown;".into()),
                Extra::Open(value) => entries.push(format!(
                    "[key: string]: {};",
                    if fields.is_empty() {
                        render(value)
                    } else {
                        "unknown".into()
                    }
                )),
            }
            format!("{{ {} }}", entries.join(" "))
        }
    }
}

pub(crate) fn generate(contracts: &BTreeMap<String, Catalog>) -> String {
    let mut exports = ["Inputs", "Outputs", "InputTypes", "OutputTypes"]
        .map(|name| format!("export interface {name} {{\n"));
    for (binding, catalog) in contracts {
        for export in &mut exports {
            export.push_str(&format!("  {}: {{\n", quoted(binding)));
        }
        for (name, tool) in &catalog.tools {
            for (index, shape) in [&tool.input, &tool.output].into_iter().enumerate() {
                exports[index].push_str(&format!(
                    "    {}: {};\n",
                    quoted(name),
                    render(&shape.value)
                ));
                let named = shape
                    .named
                    .iter()
                    .map(|(name, value)| format!("{}: {};", quoted(name), render(value)))
                    .collect::<Vec<_>>()
                    .join(" ");
                exports[index + 2].push_str(&format!("    {}: {{ {named} }};\n", quoted(name)));
            }
        }
        for export in &mut exports {
            export.push_str("  };\n");
        }
    }
    exports.map(|export| export + "}\n").join("")
}
