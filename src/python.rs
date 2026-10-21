use crate::{
    Catalog, quoted,
    schema::{Extra, Type},
};
use serde_json::Value;
use std::collections::BTreeMap;

pub(crate) fn identifier(name: &str) -> bool {
    name.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && ![
            "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
            "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global",
            "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise",
            "return", "try", "while", "with", "yield",
        ]
        .contains(&name)
}

fn block(header: &str, lines: &[String], indent: &str) -> String {
    format!(
        "{header}\n{}",
        if lines.is_empty() {
            format!("{indent}pass")
        } else {
            lines.join("\n")
        }
    )
}

fn overloaded(lines: Vec<String>) -> Vec<String> {
    if lines.len() > 1 {
        lines
            .into_iter()
            .map(|s| format!("    @_typing.overload\n{s}"))
            .collect()
    } else {
        lines
    }
}

#[derive(Default)]
struct Emitter {
    shapes: Vec<String>,
}

impl Emitter {
    fn render(&mut self, value: &Type) -> String {
        match value {
            Type::Unknown => "_builtins.object".into(),
            Type::Never => "_typing.Never".into(),
            Type::String => "_builtins.str".into(),
            Type::Integer => "_builtins.int".into(),
            Type::Number => "_builtins.float".into(),
            Type::Boolean => "_builtins.bool".into(),
            Type::Null => "None".into(),
            Type::Literal(value) => match value {
                Value::String(s) => format!("_typing.Literal[{}]", quoted(s)),
                Value::Bool(b) => format!("_typing.Literal[{}]", if *b { "True" } else { "False" }),
                Value::Null => "None".into(),
                Value::Number(n) if n.is_i64() || n.is_u64() => format!("_typing.Literal[{n}]"),
                Value::Number(_) => "_builtins.float".into(),
                _ => "_builtins.object".into(),
            },
            Type::Array(item) => format!("_builtins.list[{}]", self.render(item)),
            Type::Tuple { .. } => "_builtins.list[_builtins.object]".into(),
            Type::Union(items) => {
                if items.is_empty() {
                    "_typing.Never".into()
                } else {
                    items
                        .iter()
                        .map(|v| self.render(v))
                        .collect::<Vec<_>>()
                        .join(" | ")
                }
            }
            Type::Intersection(items) => {
                let mut combined = BTreeMap::new();
                for item in items {
                    let Type::Object { fields, .. } = item else {
                        return "_builtins.object".into();
                    };
                    for (key, field) in fields {
                        if let Some(previous) = combined.get(key)
                            && previous != field
                        {
                            return "_builtins.object".into();
                        }
                        combined.insert(key.clone(), field.clone());
                    }
                }
                self.render(&Type::Object {
                    fields: combined,
                    extra: Extra::Implicit,
                })
            }
            Type::Object { fields, extra } => {
                if fields.is_empty() || matches!(extra, Extra::Open(_)) {
                    let value = match (fields.is_empty(), extra) {
                        (true, Extra::Open(v)) => self.render(v),
                        _ => "_builtins.object".into(),
                    };
                    return format!("_builtins.dict[_builtins.str, {value}]");
                }
                let fields = fields
                    .iter()
                    .map(|(key, field)| {
                        let value = self.render(&field.value);
                        format!(
                            "{}: {}",
                            quoted(key),
                            if field.required {
                                value
                            } else {
                                format!("_typing.NotRequired[{value}]")
                            }
                        )
                    })
                    .collect::<Vec<_>>();
                let name = format!("_Shape{}", self.shapes.len());
                self.shapes.push(format!(
                    "{name} = _typing.TypedDict({name:?}, {{{}}})",
                    fields.join(", ")
                ));
                name
            }
        }
    }
}

pub(crate) fn generate(contracts: &BTreeMap<String, Catalog>) -> String {
    let mut emitter = Emitter::default();
    let mut clients = Vec::new();
    let mut exports =
        ["Inputs", "Outputs", "InputTypes", "OutputTypes"].map(|name| (name, Vec::new()));
    let mut fields = Vec::new();
    let mut indices = Vec::new();
    for (index, (binding, catalog)) in contracts.iter().enumerate() {
        let mut methods = Vec::new();
        let mut calls = Vec::new();
        let mut app_exports: [Vec<String>; 4] = Default::default();
        for (name, tool) in &catalog.tools {
            let shapes = [&tool.input, &tool.output];
            let [input, output] = shapes.map(|shape| emitter.render(&shape.value));
            calls.push(format!("    async def call_tool(self, name: _typing.Literal[{}], arguments: {input}, *, timeout: _builtins.float | None = ..., input_responses: _InputResponses | None = ..., request_state: _builtins.str | None = ..., meta: _RequestParamsMeta | None = ...) -> _ToolResult[{output}]: ...", quoted(name)));
            if identifier(name) {
                methods.push(format!("    async def {name}(self, arguments: {input}, *, timeout: _builtins.float | None = ...) -> {output}: ..."));
                for (i, (shape, value)) in shapes.into_iter().zip([input, output]).enumerate() {
                    app_exports[i].push(format!("        {name}: _typing.TypeAlias = {value}"));
                    let types = shape
                        .named
                        .iter()
                        .filter(|(name, _)| identifier(name))
                        .map(|(name, value)| {
                            format!(
                                "            {name}: _typing.TypeAlias = {}",
                                emitter.render(value)
                            )
                        })
                        .collect::<Vec<_>>();
                    app_exports[i + 2].push(block(
                        &format!("        class {name}:"),
                        &types,
                        "            ",
                    ));
                }
            }
        }
        clients.push(block(
            &format!("class _Tools{index}(_typing.Protocol):"),
            &methods,
            "    ",
        ));
        let mut app = vec![format!("    tools: _Tools{index}")];
        app.extend(overloaded(calls));
        clients.push(block(
            &format!("class _App{index}(_typing.Protocol):"),
            &app,
            "    ",
        ));
        if identifier(binding) {
            fields.push(format!("    {binding}: _App{index}"));
            for ((_, bindings), members) in exports.iter_mut().zip(app_exports) {
                bindings.push(block(
                    &format!("    class {binding}:"),
                    &members,
                    "        ",
                ));
            }
        }
        indices.push(format!(
            "    def __getitem__(self, name: _typing.Literal[{}]) -> _App{index}: ...",
            quoted(binding)
        ));
    }
    let mut sections = vec!["import builtins as _builtins\nimport typing as _typing".into()];
    sections.extend(emitter.shapes);
    sections.extend(
        exports.map(|(name, bindings)| block(&format!("class {name}:"), &bindings, "    ")),
    );
    sections.extend(clients);
    fields.extend(overloaded(indices));
    sections.push(block("class _Clients(_typing.Protocol):", &fields, "    "));
    sections.join("\n\n") + "\n"
}
