use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum ParseError {
    Io(std::io::Error),
    Json(serde_json::Error),
    UnknownSubmissionType(String),
    InvalidContent(String),
    NoInput,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "IO error: {}", err),
            Self::Json(err) => write!(f, "JSON serialization error: {}", err),
            Self::UnknownSubmissionType(s) => write!(f, "Unknown submission type: {}", s),
            Self::InvalidContent(s) => write!(f, "Invalid content: {}", s),
            Self::NoInput => write!(f, "Either filepath or content must be provided"),
        }
    }
}

impl Error for ParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Json(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ParseError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<serde_json::Error> for ParseError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MetadataValue {
    Text(String),
    List(Vec<MetadataValue>),
    Dict(HashMap<String, MetadataValue>),
}

impl MetadataValue {
    pub fn as_text(&self) -> Option<&String> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_dict(&self) -> Option<&HashMap<String, MetadataValue>> {
        match self {
            Self::Dict(dict) => Some(dict),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&Vec<MetadataValue>> {
        match self {
            Self::List(list) => Some(list),
            _ => None,
        }
    }
}

pub type MetadataDict = HashMap<String, MetadataValue>;

#[derive(Debug, Clone, PartialEq)]
pub enum SubmissionType {
    DashedDefault,
    TabPrivacy,
    TabDefault,
}

#[derive(Debug)]
pub struct DocumentIndex {
    pub document_positions: Vec<(usize, usize)>,
    pub text_positions: Vec<(usize, usize)>,
    pub header_end: usize,
    pub text_leftovers: HashMap<usize, String>,
}

impl DocumentIndex {
    pub fn new() -> Self {
        Self {
            document_positions: Vec::new(),
            text_positions: Vec::new(),
            header_end: 0,
            text_leftovers: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct DocumentInfo {
    pub filename: PathBuf,
    pub content: Vec<u8>,
    pub metadata: MetadataDict,
}
