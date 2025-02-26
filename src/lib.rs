#[macro_use]
extern crate lazy_static;

pub mod benchmark;
pub mod sgml_extractor;
pub mod sgml_parser;

#[cfg(feature = "python")]
pub mod python;

pub use sgml_extractor::*;
pub use sgml_parser::*;

#[cfg(feature = "python")]
use pyo3::prelude::*;

#[cfg(feature = "python")]
#[pymodule]
fn _rust_sgml(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(python::parse_sgml_submission, m)?)?;
    m.add_function(wrap_pyfunction!(python::extract_sgml_to_directory, m)?)?;
    m.add_function(wrap_pyfunction!(python::benchmark_directory, m)?)?;
    Ok(())
}
