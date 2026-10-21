use crate::{
    Error,
    canonical::canonical,
    contract::{Contract, Expectation, Identity, Metadata},
};
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Change {
    NoContract,
    Identity {
        expected: Identity,
        published: Identity,
    },
    ToolRemoved {
        name: String,
    },
    ToolInputChanged {
        name: String,
    },
    ToolOutputChanged {
        name: String,
    },
    ResourceRemoved {
        uri: String,
    },
    ResourceOutputChanged {
        uri: String,
    },
}

impl fmt::Display for Change {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoContract => write!(formatter, "no published contract"),
            Self::Identity {
                expected,
                published,
            } => write!(
                formatter,
                "identity {}@{} expected, {}@{} published",
                expected.name, expected.version, published.name, published.version
            ),
            Self::ToolRemoved { name } => write!(formatter, "tool {name} removed"),
            Self::ToolInputChanged { name } => write!(formatter, "tool {name} input changed"),
            Self::ToolOutputChanged { name } => write!(formatter, "tool {name} output changed"),
            Self::ResourceRemoved { uri } => write!(formatter, "resource {uri} removed"),
            Self::ResourceOutputChanged { uri } => {
                write!(formatter, "resource {uri} contents changed")
            }
        }
    }
}

pub fn changes(expected: &Expectation, published: Option<&Metadata>) -> Vec<Change> {
    let Some(published_contract) = published.and_then(|metadata| metadata.contract.as_ref()) else {
        return vec![Change::NoContract];
    };
    let published = published.expect("contract is present when metadata is present");
    let mut result = Vec::new();
    if expected.identity != published.identity {
        result.push(Change::Identity {
            expected: expected.identity.clone(),
            published: published.identity.clone(),
        });
    }
    compare_contract(&expected.contract, published_contract, &mut result);
    result
}

pub fn changes_json(input: &str) -> Result<String, Error> {
    #[derive(Deserialize)]
    struct Request {
        expected: Expectation,
        published: Option<Metadata>,
    }

    let request: Request = serde_json::from_str(input)?;
    request.expected.validate().map_err(Error::Contract)?;
    if let Some(published) = &request.published {
        published.validate().map_err(Error::Contract)?;
    }
    let records = changes(&request.expected, request.published.as_ref())
        .into_iter()
        .map(|change| {
            let mut value = serde_json::to_value(&change).expect("changes are serializable");
            if let Value::Object(object) = &mut value {
                object.insert("text".into(), Value::String(change.to_string()));
            }
            value
        })
        .collect::<Vec<_>>();
    Ok(serde_json::to_string(
        &serde_json::json!({"changes": records}),
    )?)
}

fn compare_contract(expected: &Contract, published: &Contract, result: &mut Vec<Change>) {
    for (name, expected_tool) in &expected.tools {
        let Some(published_tool) = published.tools.get(name) else {
            result.push(Change::ToolRemoved { name: name.clone() });
            continue;
        };
        if canonical(&expected_tool.input) != canonical(&published_tool.input) {
            result.push(Change::ToolInputChanged { name: name.clone() });
        } else if !same_optional_schema(&expected_tool.output, &published_tool.output) {
            result.push(Change::ToolOutputChanged { name: name.clone() });
        }
    }
    for (uri, expected_resource) in &expected.resources {
        let Some(published_resource) = published.resources.get(uri) else {
            result.push(Change::ResourceRemoved { uri: uri.clone() });
            continue;
        };
        if canonical(&expected_resource.output) != canonical(&published_resource.output) {
            result.push(Change::ResourceOutputChanged { uri: uri.clone() });
        }
    }
}

fn same_optional_schema(
    left: &Option<crate::contract::Schema>,
    right: &Option<crate::contract::Schema>,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => canonical(left) == canonical(right),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{Resource, Schema, Tool};
    use serde_json::json;
    use std::collections::BTreeMap;

    fn expectation() -> Expectation {
        Expectation {
            identity: Identity {
                name: "example/orders".into(),
                version: "1".into(),
            },
            contract: Contract {
                tools: BTreeMap::from([(
                    "get".into(),
                    Tool {
                        input: Schema(
                            json!({"type":"object","properties":{"id":{"type":"string","description":"before"}},"required":["id"]}),
                        ),
                        output: None,
                    },
                )]),
                resources: BTreeMap::from([(
                    "orders://summary".into(),
                    Resource {
                        output: Schema(
                            json!({"type":"object","properties":{"total":{"type":"number"}}}),
                        ),
                    },
                )]),
            },
        }
    }

    fn metadata() -> Metadata {
        let expected = expectation();
        Metadata {
            version: 1,
            identity: expected.identity.clone(),
            contract: Some(Contract {
                tools: BTreeMap::from([
                    (
                        "get".into(),
                        Tool {
                            input: Schema(
                                json!({"required":["id"],"properties":{"id":{"description":"after","type":"string"}},"type":"object"}),
                            ),
                            output: None,
                        },
                    ),
                    (
                        "new".into(),
                        Tool {
                            input: Schema(json!({"type":"object"})),
                            output: None,
                        },
                    ),
                ]),
                resources: BTreeMap::from([
                    (
                        "orders://summary".into(),
                        Resource {
                            output: Schema(
                                json!({"properties":{"total":{"type":"number","description":"total"}},"type":"object"}),
                            ),
                        },
                    ),
                    (
                        "orders://new".into(),
                        Resource {
                            output: Schema(json!({"type":"object"})),
                        },
                    ),
                ]),
            }),
            requires: BTreeMap::new(),
        }
    }

    #[test]
    fn documentation_and_additive_contracts_are_compatible() {
        assert!(changes(&expectation(), Some(&metadata())).is_empty());
    }

    #[test]
    fn missing_contract_identity_and_sorted_breaking_changes_are_reported() {
        assert_eq!(changes(&expectation(), None), vec![Change::NoContract]);
        let mut actual = metadata();
        actual.identity.version = "2".into();
        let contract = actual.contract.as_mut().unwrap();
        contract.tools.remove("get");
        contract.resources.insert(
            "orders://summary".into(),
            Resource {
                output: Schema(json!({"type":"array"})),
            },
        );
        assert_eq!(
            changes(&expectation(), Some(&actual)),
            vec![
                Change::Identity {
                    expected: Identity {
                        name: "example/orders".into(),
                        version: "1".into()
                    },
                    published: Identity {
                        name: "example/orders".into(),
                        version: "2".into()
                    }
                },
                Change::ToolRemoved { name: "get".into() },
                Change::ResourceOutputChanged {
                    uri: "orders://summary".into()
                },
            ]
        );
    }

    #[test]
    fn changes_keep_holon_display_text() {
        assert_eq!(
            Change::ToolInputChanged { name: "get".into() }.to_string(),
            "tool get input changed"
        );
        assert_eq!(
            Change::ResourceOutputChanged {
                uri: "orders://summary".into()
            }
            .to_string(),
            "resource orders://summary contents changed"
        );
    }
}
