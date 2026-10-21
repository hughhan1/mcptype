//! JSON-in, JSON-out entry points shared by the C, Node, and Python bindings.
use crate::{Contract, Error, Expectation, Metadata, Schema, canonical_contract, digest};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;

pub fn compile(input: &[u8], cancelled: &AtomicBool) -> Result<String, Error> {
    Ok(serde_json::to_string(&crate::compile_cancellable(
        serde_json::from_slice(input)?,
        cancelled,
    )?)?)
}

/// `{"contract": Contract}` or `{"schema": {...}}` to `{"canonical", "digest"}`.
pub fn canonical(input: &[u8]) -> Result<String, Error> {
    #[derive(Deserialize)]
    #[serde(rename_all = "lowercase")]
    enum Request {
        Contract(Contract),
        Schema(Schema),
    }
    let (canonical, digest) = match serde_json::from_slice(input)? {
        Request::Contract(contract) => (
            serde_json::to_value(canonical_contract(&contract))?,
            digest(&contract),
        ),
        Request::Schema(schema) => {
            let schema = Value::Object(crate::canonical(&schema));
            let digest = crate::canonical::hash(&schema);
            (schema, digest)
        }
    };
    Ok(serde_json::to_string(
        &json!({"canonical": canonical, "digest": digest}),
    )?)
}

/// `{"expected": Expectation, "published": Metadata | null}` to `{"changes": [{"kind", ..., "text"}]}`.
pub fn changes(input: &[u8]) -> Result<String, Error> {
    #[derive(Deserialize)]
    struct Request {
        expected: Expectation,
        published: Option<Metadata>,
    }
    let request: Request = serde_json::from_slice(input)?;
    let changes = crate::changes(&request.expected, request.published.as_ref())
        .into_iter()
        .map(|change| {
            let mut record = serde_json::to_value(&change)?;
            record["text"] = change.to_string().into();
            Ok(record)
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(serde_json::to_string(&json!({"changes": changes}))?)
}
