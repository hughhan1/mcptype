use std::sync::atomic::AtomicBool;

fn run(result: Result<String, mcptype::Error>) -> napi::Result<String> {
    result.map_err(|error| napi::Error::from_reason(error.to_string()))
}

#[napi_derive::napi]
pub fn compile(input: String) -> napi::Result<String> {
    run(mcptype::json::compile(
        input.as_bytes(),
        &AtomicBool::new(false),
    ))
}

#[napi_derive::napi]
pub fn canonical(input: String) -> napi::Result<String> {
    run(mcptype::json::canonical(input.as_bytes()))
}

#[napi_derive::napi]
pub fn changes(input: String) -> napi::Result<String> {
    run(mcptype::json::changes(input.as_bytes()))
}

#[napi_derive::napi]
pub fn generate(input: String) -> napi::Result<String> {
    run(mcptype::json::generate(input.as_bytes()))
}
