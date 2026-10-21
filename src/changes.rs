use crate::{
    canonical::canonical_contract,
    contract::{Expectation, Identity, Metadata},
};
use serde::Serialize;
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

/// What breaks `expected` if `published` is deployed. Empty means compatible.
/// Identity first, then tools by name, then resources by URI.
pub fn changes(expected: &Expectation, published: Option<&Metadata>) -> Vec<Change> {
    let Some((metadata, contract)) =
        published.and_then(|metadata| Some((metadata, metadata.contract.as_ref()?)))
    else {
        return vec![Change::NoContract];
    };
    let mut result = Vec::new();
    if expected.identity != metadata.identity {
        result.push(Change::Identity {
            expected: expected.identity.clone(),
            published: metadata.identity.clone(),
        });
    }
    let (expected, published) = (
        canonical_contract(&expected.contract),
        canonical_contract(contract),
    );
    for (name, tool) in expected.tools {
        result.extend(match published.tools.get(&name) {
            None => Some(Change::ToolRemoved { name }),
            Some(actual) if actual.input != tool.input => Some(Change::ToolInputChanged { name }),
            Some(actual) if actual.output != tool.output => {
                Some(Change::ToolOutputChanged { name })
            }
            Some(_) => None,
        });
    }
    for (uri, resource) in expected.resources {
        result.extend(match published.resources.get(&uri) {
            None => Some(Change::ResourceRemoved { uri }),
            Some(actual) if actual.output != resource.output => {
                Some(Change::ResourceOutputChanged { uri })
            }
            Some(_) => None,
        });
    }
    result
}
