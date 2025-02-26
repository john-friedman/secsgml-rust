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
use pyo3::wrap_pymodule;

#[cfg(feature = "python")]
use pyo3::prelude::*;

#[cfg(feature = "python")]
#[pymodule]
fn rust_sgml(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_wrapped(wrap_pymodule!(python::rust_sgml))?;
    Ok(())
}
