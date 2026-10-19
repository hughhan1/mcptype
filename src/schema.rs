use crate::{Error, check_cancelled};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;

#[derive(Default)]
pub(crate) struct Shape {
    pub value: Type,
    pub named: BTreeMap<String, Type>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) enum Type {
    #[default]
    Unknown,
    Never,
    String,
    Integer,
    Number,
    Boolean,
    Null,
    Literal(Value),
    Array(Box<Type>),
    Tuple {
        prefix: Vec<Type>,
        required: usize,
        rest: Extra,
    },
    Object {
        fields: BTreeMap<String, Field>,
        extra: Extra,
    },
    Union(Vec<Type>),
    Intersection(Vec<Type>),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Extra {
    Implicit,
    Closed,
    Open(Box<Type>),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Field {
    pub value: Type,
    pub required: bool,
}

impl Shape {
    pub fn parse(schema: &Value, cancelled: &AtomicBool) -> Result<Self, Error> {
        let mut parser = Parser {
            root: schema,
            cancelled,
            active: BTreeSet::new(),
        };
        let value = parser.parse(schema, 0)?;
        let mut named = BTreeMap::new();
        for section in ["definitions", "$defs"] {
            if let Some(definitions) = schema.get(section).and_then(Value::as_object) {
                for (name, schema) in definitions {
                    named.insert(name.clone(), parser.parse(schema, 0)?);
                }
            }
        }
        Ok(Self { value, named })
    }
}

struct Parser<'a> {
    root: &'a Value,
    cancelled: &'a AtomicBool,
    active: BTreeSet<String>,
}

impl Parser<'_> {
    fn extra(&mut self, value: Option<&Value>, depth: usize) -> Result<Extra, Error> {
        Ok(match value {
            None => Extra::Implicit,
            Some(Value::Bool(false)) => Extra::Closed,
            Some(value) => Extra::Open(Box::new(self.parse(value, depth + 1)?)),
        })
    }

    fn parse(&mut self, schema: &Value, depth: usize) -> Result<Type, Error> {
        check_cancelled(self.cancelled)?;
        if depth > 128 {
            return Ok(Type::Unknown);
        }
        let Some(object) = schema.as_object() else {
            return match schema {
                Value::Bool(false) => Ok(Type::Never),
                Value::Bool(true) => Ok(Type::Unknown),
                _ => Err(Error::Contract(
                    "schema must be an object or boolean".into(),
                )),
            };
        };
        if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
            let Some(pointer) = reference.strip_prefix('#') else {
                return Err(Error::Contract(format!(
                    "external schema reference is unsupported: {reference}"
                )));
            };
            let target = self.root.pointer(pointer).ok_or_else(|| {
                Error::Contract(format!("unresolved schema reference: {reference}"))
            })?;
            if !self.active.insert(reference.to_owned()) {
                return Ok(Type::Unknown);
            }
            let value = self.parse(target, depth + 1)?;
            self.active.remove(reference);
            return Ok(value);
        }
        for (key, intersection) in [("anyOf", false), ("oneOf", false), ("allOf", true)] {
            if let Some(items) = object.get(key).and_then(Value::as_array) {
                let types = items
                    .iter()
                    .map(|item| self.parse(item, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;
                return Ok(if intersection {
                    Type::Intersection(types)
                } else {
                    Type::Union(types)
                });
            }
        }
        if let Some(value) = object.get("const") {
            return Ok(Type::Literal(value.clone()));
        }
        if let Some(values) = object.get("enum").and_then(Value::as_array) {
            return Ok(Type::Union(
                values.iter().cloned().map(Type::Literal).collect(),
            ));
        }
        if let Some(kinds) = object.get("type").and_then(Value::as_array) {
            let mut types = Vec::new();
            for kind in kinds {
                let mut schema = schema.clone();
                schema["type"] = kind.clone();
                types.push(self.parse(&schema, depth + 1)?);
            }
            return Ok(Type::Union(types));
        }
        Ok(match object.get("type").and_then(Value::as_str) {
            Some("string") => Type::String,
            Some("integer") => Type::Integer,
            Some("number") => Type::Number,
            Some("boolean") => Type::Boolean,
            Some("null") => Type::Null,
            Some("array") => {
                if let Some(prefix) = object
                    .get("prefixItems")
                    .or_else(|| object.get("items").filter(|v| v.is_array()))
                    .and_then(Value::as_array)
                {
                    let items = prefix
                        .iter()
                        .map(|item| self.parse(item, depth + 1))
                        .collect::<Result<Vec<_>, _>>()?;
                    let rest = self.extra(
                        object.get(if object.contains_key("prefixItems") {
                            "items"
                        } else {
                            "additionalItems"
                        }),
                        depth,
                    )?;
                    Type::Tuple {
                        prefix: items,
                        required: object.get("minItems").and_then(Value::as_u64).unwrap_or(0)
                            as usize,
                        rest,
                    }
                } else {
                    Type::Array(Box::new(self.parse(
                        object.get("items").unwrap_or(&Value::Bool(true)),
                        depth + 1,
                    )?))
                }
            }
            Some("object") | None
                if object.contains_key("properties")
                    || object.get("type").and_then(Value::as_str) == Some("object") =>
            {
                let required = object.get("required").and_then(Value::as_array);
                let mut fields = BTreeMap::new();
                if let Some(properties) = object.get("properties").and_then(Value::as_object) {
                    for (key, value) in properties {
                        fields.insert(
                            key.clone(),
                            Field {
                                value: self.parse(value, depth + 1)?,
                                required: required
                                    .is_some_and(|r| r.contains(&Value::String(key.clone()))),
                            },
                        );
                    }
                }
                Type::Object {
                    fields,
                    extra: self.extra(object.get("additionalProperties"), depth)?,
                }
            }
            _ => Type::Unknown,
        })
    }
}
