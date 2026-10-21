#[pyo3::pymodule]
mod _native {
    use pyo3::{exceptions::PyValueError, prelude::*};

    #[pyfunction]
    fn compile(input: &str) -> PyResult<String> {
        mcptype::compile_json(input).map_err(|error| PyValueError::new_err(error.to_string()))
    }

    #[pyfunction]
    fn canonical(input: &str) -> PyResult<String> {
        mcptype::canonical_json(input).map_err(|error| PyValueError::new_err(error.to_string()))
    }

    #[pyfunction]
    fn changes(input: &str) -> PyResult<String> {
        mcptype::changes_json(input).map_err(|error| PyValueError::new_err(error.to_string()))
    }

    #[pyfunction]
    fn generate(input: &str) -> PyResult<String> {
        mcptype::generate_json(input).map_err(|error| PyValueError::new_err(error.to_string()))
    }
}
