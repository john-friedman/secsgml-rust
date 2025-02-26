use crate::sgml_parser::{parse_sgml_submission, MetadataDict, MetadataValue, SgmlParserError};
use serde_json;
use std::fs;
use std::path::Path;

/// Extract SGML submission contents and write to directory structure
pub fn extract_sgml_to_directory(
    content: Option<&str>,
    filepath: Option<&str>,
    output_dir: &str,
) -> Result<(), SgmlParserError> {
    // Validate input
    if content.is_none() && filepath.is_none() {
        return Err(SgmlParserError::IoError(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Either content or filepath must be provided",
        )));
    }

    // Create output directory if it doesn't exist
    fs::create_dir_all(output_dir)?;

    // Parse SGML content
    let content_str = content.unwrap_or("");
    let (header_metadata, documents) = match (content, filepath) {
        (Some(c), _) => parse_sgml_submission(c, None)?,
        (_, Some(f)) => parse_sgml_submission("", Some(f))?,
        _ => unreachable!(), // We already checked this above
    };

    // Get accession number
    let accn = if let Some(MetadataValue::String(accn)) = header_metadata.get("accession-number") {
        accn.clone()
    } else if let Some(MetadataValue::String(accn)) = header_metadata.get("accession number") {
        accn.clone()
    } else {
        return Err(SgmlParserError::IoError(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Could not find accession number in metadata",
        )));
    };

    // Create directory for this submission
    let submission_dir = format!("{}/{}", output_dir, accn);
    fs::create_dir_all(&submission_dir)?;

    // Write metadata to JSON file
    let metadata_path = format!("{}/metadata.json", submission_dir);
    let metadata_json = serde_json::to_string_pretty(&header_metadata)?;
    fs::write(metadata_path, metadata_json)?;

    // Extract documents info
    let documents_metadata = match header_metadata.get("documents") {
        Some(MetadataValue::List(docs)) => docs,
        _ => {
            return Err(SgmlParserError::IoError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Could not find documents metadata",
            )))
        }
    };

    // Write each document to a file
    for (idx, doc_metadata) in documents_metadata.iter().enumerate() {
        if idx >= documents.len() {
            continue; // Skip if no corresponding document data
        }

        let filename = match doc_metadata {
            MetadataValue::Dict(metadata) => {
                if let Some(MetadataValue::String(filename)) = metadata.get("filename") {
                    filename.clone()
                } else if let Some(MetadataValue::String(sequence)) = metadata.get("sequence") {
                    format!("{}.txt", sequence)
                } else {
                    format!("document_{}.txt", idx + 1)
                }
            }
            _ => format!("document_{}.txt", idx + 1),
        };

        let file_path = format!("{}/{}", submission_dir, filename);
        fs::write(file_path, &documents[idx])?;
    }

    Ok(())
}
