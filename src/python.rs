use crate::benchmark::benchmark_directory as rust_benchmark;
use crate::sgml_extractor::extract_sgml_to_directory as rust_extract;
use crate::sgml_parser::{parse_sgml_submission as rust_parse, MetadataValue, SgmlParserError};
use pyo3::exceptions::{PyIOError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyString};
use std::collections::HashMap;

fn convert_error(err: SgmlParserError) -> PyErr {
    match err {
        SgmlParserError::IoError(io_err) => PyIOError::new_err(format!("IO Error: {}", io_err)),
        SgmlParserError::UUDecodeError(msg) => {
            PyValueError::new_err(format!("UUDecode Error: {}", msg))
        }
        SgmlParserError::JsonError(json_err) => {
            PyValueError::new_err(format!("JSON Error: {}", json_err))
        }
        SgmlParserError::NoBeginLine => PyValueError::new_err("No valid begin line found"),
        SgmlParserError::UnknownSubmissionType => PyValueError::new_err("Unknown submission type"),
    }
}

fn metadata_to_pyobject(py: Python, value: &MetadataValue) -> PyResult<PyObject> {
    match value {
        MetadataValue::String(s) => Ok(s.clone().into_py(py)),
        MetadataValue::List(list) => {
            let py_list = PyList::empty(py);
            for item in list {
                py_list.append(metadata_to_pyobject(py, item)?)?;
            }
            Ok(py_list.into_py(py))
        }
        MetadataValue::Dict(dict) => {
            let py_dict = PyDict::new(py);
            for (key, val) in dict {
                py_dict.set_item(key, metadata_to_pyobject(py, val)?)?;
            }
            Ok(py_dict.into_py(py))
        }
    }
}

#[pyfunction]
pub fn parse_sgml_submission(
    py: Python,
    content: &str,
    filepath: Option<&str>,
) -> PyResult<(PyObject, Vec<PyObject>)> {
    let result = match rust_parse(content, filepath) {
        Ok(result) => result,
        Err(err) => return Err(convert_error(err)),
    };

    let (metadata, documents) = result;

    let py_metadata = metadata_to_pyobject(py, &MetadataValue::Dict(metadata))?;

    let py_documents = documents
        .into_iter()
        .map(|doc| PyBytes::new(py, &doc).into_py(py))
        .collect();

    Ok((py_metadata, py_documents))
}

#[pyfunction]
pub fn extract_sgml_to_directory(
    content: &str,
    filepath: Option<&str>,
    output_dir: &str,
) -> PyResult<()> {
    rust_extract(Some(content), filepath, output_dir).map_err(convert_error)
}

#[pyfunction]
pub fn benchmark_directory(dir_path: &str, output_file: &str) -> PyResult<()> {
    rust_benchmark(dir_path, output_file)
        .map_err(|e| PyIOError::new_err(format!("Benchmark error: {}", e)))
}
