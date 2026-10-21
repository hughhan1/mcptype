#[pyo3::pymodule]
mod _native {
    use pyo3::{exceptions::PyValueError, prelude::*};
    use std::sync::atomic::AtomicBool;

    fn run(result: Result<String, mcptype::Error>) -> PyResult<String> {
        result.map_err(|error| PyValueError::new_err(error.to_string()))
    }

    #[pyfunction]
    fn compile(input: &str) -> PyResult<String> {
        run(mcptype::json::compile(
            input.as_bytes(),
            &AtomicBool::new(false),
        ))
    }

    #[pyfunction]
    fn canonical(input: &str) -> PyResult<String> {
        run(mcptype::json::canonical(input.as_bytes()))
    }

    #[pyfunction]
    fn changes(input: &str) -> PyResult<String> {
        run(mcptype::json::changes(input.as_bytes()))
    }
}
