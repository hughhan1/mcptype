#[pyo3::pymodule]
mod _native {
    use pyo3::{exceptions::PyValueError, prelude::*};

    #[pyfunction]
    fn compile(input: &str) -> PyResult<String> {
        mcptype::compile_json(input).map_err(|error| PyValueError::new_err(error.to_string()))
    }
}
