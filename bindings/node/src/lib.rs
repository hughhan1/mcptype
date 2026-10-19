#[napi_derive::napi]
pub fn compile(input: String) -> napi::Result<String> {
    mcptype::compile_json(&input).map_err(|error| napi::Error::from_reason(error.to_string()))
}
