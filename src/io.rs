use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::types::{DocumentInfo, MetadataDict, MetadataValue, ParseError};
use crate::utils::{default_filename, detect_uu, safe_filename};

/// Ensure output directory exists
pub fn ensure_output_dir(dir: &Path) -> Result<(), ParseError> {
    if !dir.exists() {
        fs::create_dir_all(dir).map_err(ParseError::Io)?;
    } else if !dir.is_dir() {
        return Err(ParseError::InvalidContent(format!(
            "Output path exists but is not a directory: {}",
            dir.display()
        )));
    }
    Ok(())
}

/// Write metadata to JSON file
pub fn write_metadata(metadata: &MetadataDict, output_dir: &Path) -> Result<(), ParseError> {
    let metadata_path = output_dir.join("metadata.json");
    let metadata_file = File::create(metadata_path).map_err(ParseError::Io)?;
    serde_json::to_writer_pretty(metadata_file, metadata).map_err(ParseError::Json)?;
    Ok(())
}

/// Extract filename from document metadata
fn extract_filename(doc_metadata: &MetadataDict, index: usize, is_binary: bool) -> String {
    // Try to get filename from metadata
    if let Some(MetadataValue::Text(filename)) = doc_metadata.get("filename") {
        return safe_filename(filename);
    }

    // Try to derive from document type
    if let Some(MetadataValue::Text(doc_type)) = doc_metadata.get("type") {
        let doc_type = doc_type.to_lowercase();
        let extension = match doc_type.as_str() {
            "10-k" | "10-q" | "8-k" => "txt",
            "ex-101" => "xml",
            "graphic" => "jpg",
            _ => {
                if is_binary {
                    "bin"
                } else {
                    "txt"
                }
            }
        };

        return format!("{}_{}.{}", doc_type, index + 1, extension);
    }

    // Fallback to default
    default_filename(index, is_binary)
}

/// Prepare document info for writing
pub fn prepare_documents(documents: Vec<Vec<u8>>, metadata: &MetadataDict) -> Vec<DocumentInfo> {
    let mut result = Vec::new();

    // Get document metadata list
    let doc_metadata_list = match metadata.get("documents") {
        Some(MetadataValue::List(list)) => list,
        _ => return result, // No documents in metadata
    };

    for (i, (content, metadata_value)) in documents
        .into_iter()
        .zip(doc_metadata_list.iter())
        .enumerate()
    {
        let doc_metadata = match metadata_value {
            MetadataValue::Dict(dict) => dict.clone(),
            _ => continue, // Skip if not a dictionary
        };

        // Detect if content is binary (UU encoded)
        let is_binary = !content.is_empty()
            && std::str::from_utf8(&content[..std::cmp::min(10, content.len())])
                .map(|s| detect_uu(s))
                .unwrap_or(false);

        let filename = extract_filename(&doc_metadata, i, is_binary);

        result.push(DocumentInfo {
            filename: PathBuf::from(filename),
            content,
            metadata: doc_metadata,
        });
    }

    result
}

/// Write documents to output directory
pub fn write_documents(documents: Vec<DocumentInfo>, output_dir: &Path) -> Result<(), ParseError> {
    for doc in documents {
        let path = output_dir.join(doc.filename);
        let mut file = File::create(path).map_err(ParseError::Io)?;
        file.write_all(&doc.content).map_err(ParseError::Io)?;
    }
    Ok(())
}
