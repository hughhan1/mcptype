#[napi_derive::napi]
pub fn compile(input: String) -> napi::Result<String> {
    mcptype::compile_json(&input).map_err(|error| napi::Error::from_reason(error.to_string()))
}

#[napi_derive::napi]
pub fn canonical(input: String) -> napi::Result<String> {
    mcptype::canonical_json(&input).map_err(|error| napi::Error::from_reason(error.to_string()))
}

#[napi_derive::napi]
pub fn changes(input: String) -> napi::Result<String> {
    mcptype::changes_json(&input).map_err(|error| napi::Error::from_reason(error.to_string()))
}

#[napi_derive::napi]
pub fn generate(input: String) -> napi::Result<String> {
    mcptype::generate_json(&input).map_err(|error| napi::Error::from_reason(error.to_string()))
}
