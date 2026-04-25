use std::path::Path;

use pyo3::{exceptions::PyRuntimeError, prelude::*};

#[pyfunction]
#[pyo3(name = "aac_apply_gain_file")]
fn aac_apply_gain_file_py(src_path: &str, dst_path: &str, gain_steps: i32) -> PyResult<usize> {
    m4againrs::aac_apply_gain_file(Path::new(src_path), Path::new(dst_path), gain_steps)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))
}

#[pymodule]
#[pyo3(name = "m4againrs")]
fn m4againrs_module(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("__all__", vec!["aac_apply_gain_file", "GAIN_STEP_DB"])?;
    module.add("GAIN_STEP_DB", m4againrs::GAIN_STEP_DB)?;
    module.add_function(wrap_pyfunction!(aac_apply_gain_file_py, module)?)?;
    Ok(())
}
