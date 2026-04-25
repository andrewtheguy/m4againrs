use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

use pyo3::{exceptions::PyRuntimeError, prelude::*, types::PyBytes};

struct PyOutputWriter {
    output: Py<PyAny>,
}

impl Write for PyOutputWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Python::attach(|py| {
            let output = self.output.bind(py);
            let bytes = PyBytes::new(py, buf);
            let written = output
                .call_method1("write", (bytes,))
                .map_err(py_err_to_io)?;

            if !written.is_none() {
                let written = written.extract::<usize>().map_err(py_err_to_io)?;
                if written != buf.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "Python output.write returned a short write",
                    ));
                }
            }

            Ok(buf.len())
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        Python::attach(|py| {
            let output = self.output.bind(py);
            if output.hasattr("flush").map_err(py_err_to_io)? {
                output.call_method0("flush").map_err(py_err_to_io)?;
            }
            Ok(())
        })
    }
}

fn py_err_to_io(err: PyErr) -> io::Error {
    io::Error::other(err.to_string())
}

#[pyfunction]
#[pyo3(name = "aac_apply_gain_file")]
fn aac_apply_gain_file_py(src_path: &str, dst_path: &str, gain_steps: i32) -> PyResult<usize> {
    m4againrs::aac_apply_gain_file(Path::new(src_path), Path::new(dst_path), gain_steps)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))
}

#[pyfunction]
#[pyo3(name = "aac_apply_gain_to_writer")]
fn aac_apply_gain_to_writer_py(
    src_path: &str,
    output: Py<PyAny>,
    gain_steps: i32,
) -> PyResult<usize> {
    let mut src = File::open(src_path).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    let mut output = PyOutputWriter { output };
    m4againrs::aac_apply_gain_to_writer(&mut src, &mut output, gain_steps)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))
}

#[pymodule]
#[pyo3(name = "m4againrs")]
fn m4againrs_module(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add(
        "__all__",
        vec![
            "aac_apply_gain_file",
            "aac_apply_gain_to_writer",
            "GAIN_STEP_DB",
        ],
    )?;
    module.add("GAIN_STEP_DB", m4againrs::GAIN_STEP_DB)?;
    module.add_function(wrap_pyfunction!(aac_apply_gain_file_py, module)?)?;
    module.add_function(wrap_pyfunction!(aac_apply_gain_to_writer_py, module)?)?;
    Ok(())
}
