use crate::sgml_extractor::extract_sgml_to_directory;
use crate::sgml_parser::{parse_sgml_submission, MetadataDict, MetadataValue, SgmlParserError};
use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyIOError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};
use std::collections::HashMap;

// Create custom exception for SGML parser errors
create_exception!(secsgml, SGMLParserError, PyException);

// Convert Rust SgmlParserError to Python exceptions
fn convert_error(err: SgmlParserError) -> PyErr {
    match err {
        SgmlParserError::IoError(io_err) => PyIOError::new_err(io_err.to_string()),
        SgmlParserError::UUDecodeError(msg) => PyValueError::new_err(msg),
        SgmlParserError::JsonError(json_err) => PyValueError::new_err(json_err.to_string()),
        SgmlParserError::NoBeginLine => PyValueError::new_err("No valid begin line found"),
        SgmlParserError::UnknownSubmissionType => PyValueError::new_err("Unknown submission type"),
    }
}

// Convert Rust MetadataValue to Python objects
fn metadata_value_to_py(py: Python, value: &MetadataValue) -> PyResult<PyObject> {
    match value {
        MetadataValue::String(s) => Ok(s.clone().into_py(py)),
        MetadataValue::List(items) => {
            let list = PyList::empty(py);
            for item in items {
                list.append(metadata_value_to_py(py, item)?)?;
            }
            Ok(list.into())
        }
        MetadataValue::Dict(dict) => {
            let py_dict = PyDict::new(py);
            for (key, val) in dict {
                py_dict.set_item(key, metadata_value_to_py(py, val)?)?;
            }
            Ok(py_dict.into())
        }
    }
}

// Function that matches the existing parse_sgml_submission_into_memory signature
#[pyfunction]
fn parse_sgml_submission_into_memory(
    py: Python,
    content: &str,
) -> PyResult<(PyObject, Vec<PyObject>)> {
    let result = parse_sgml_submission(content, None).map_err(convert_error)?;

    let (metadata, documents) = result;

    // Convert metadata to Python dict
    let py_metadata = metadata_value_to_py(py, &MetadataValue::Dict(metadata))?;

    // Convert documents to Python bytes
    let py_documents = documents
        .into_iter()
        .map(|doc| PyBytes::new(py, &doc).into_py(py))
        .collect::<Vec<PyObject>>();

    Ok((py_metadata, py_documents))
}

// Function that matches the existing parse_sgml_submission signature
#[pyfunction]
fn parse_sgml_submission(
    py: Python,
    content: Option<&str>,
    filepath: Option<&str>,
) -> PyResult<(PyObject, Vec<PyObject>)> {
    let result = crate::sgml_parser::parse_sgml_submission(content.unwrap_or(""), filepath)
        .map_err(convert_error)?;

    let (metadata, documents) = result;

    // Convert metadata to Python dict
    let py_metadata = metadata_value_to_py(py, &MetadataValue::Dict(metadata))?;

    // Convert documents to Python bytes
    let py_documents = documents
        .into_iter()
        .map(|doc| PyBytes::new(py, &doc).into_py(py))
        .collect::<Vec<PyObject>>();

    Ok((py_metadata, py_documents))
}

#[pyfunction]
fn extract_sgml(content: Option<&str>, filepath: Option<&str>, output_dir: &str) -> PyResult<()> {
    extract_sgml_to_directory(content, filepath, output_dir).map_err(convert_error)
}

/// SGML Parser module for SEC EDGAR filings
#[pymodule]
fn rust_sgml(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse_sgml_submission_into_memory, m)?)?;
    m.add_function(wrap_pyfunction!(parse_sgml_submission, m)?)?;
    m.add_function(wrap_pyfunction!(extract_sgml, m)?)?;
    m.add("SGMLParserError", _py.get_type::<SGMLParserError>())?;
    Ok(())
}
