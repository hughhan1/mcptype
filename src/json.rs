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

/// `{"contract": Contract}`, `{"schema": {...}}` or `{"metadata": Metadata}` to
/// `{"canonical", "digest"}`; both are `null` for metadata without a contract.
pub fn canonical(input: &[u8]) -> Result<String, Error> {
    #[derive(Deserialize)]
    #[serde(rename_all = "lowercase")]
    enum Request {
        Contract(Contract),
        Schema(Schema),
        Metadata(Box<Metadata>),
    }
    let contract = |contract: &Contract| -> Result<_, Error> {
        Ok((
            serde_json::to_value(canonical_contract(contract))?,
            digest(contract).into(),
        ))
    };
    let (canonical, digest) = match serde_json::from_slice(input)? {
        Request::Contract(document) => contract(&document)?,
        Request::Schema(schema) => {
            let schema = Value::Object(crate::canonical(&schema));
            let digest = crate::canonical::hash(&schema).into();
            (schema, digest)
        }
        Request::Metadata(metadata) => match &metadata.contract {
            Some(document) => contract(document)?,
            None => (Value::Null, Value::Null),
        },
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

/// `GenerateRequest` with base64 `runtime` values to `{"files": {path: base64}}`.
pub fn generate(input: &[u8]) -> Result<String, Error> {
    Ok(serde_json::to_string(&crate::generate(
        serde_json::from_slice(input)?,
    )?)?)
}
