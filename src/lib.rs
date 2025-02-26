mod io;
mod sgml;
mod types;
mod utils;

pub use sgml::{parse_sgml_into_memory, parse_sgml_submission};
pub use types::{MetadataDict, MetadataValue, ParseError};
