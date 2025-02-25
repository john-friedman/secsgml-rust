#[macro_use]
extern crate lazy_static;

mod sgml_parser;
mod uu_decode;

pub use sgml_parser::determine_file_extension;
pub use sgml_parser::parse_sgml_submission_into_memory;
pub use uu_decode::decode as uu_decode;

// Re-export serde_json for use in main.rs
pub use serde_json;
