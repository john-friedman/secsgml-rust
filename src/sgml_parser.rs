use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use thiserror::Error;
use uuencode::uudecode;

#[derive(Error, Debug)]
pub enum SgmlParserError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("UUDecode error: {0}")]
    UUDecodeError(String),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("No valid begin line found")]
    NoBeginLine,

    #[error("Unknown submission type")]
    UnknownSubmissionType,
}

pub type MetadataDict = HashMap<String, MetadataValue>;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum MetadataValue {
    String(String),
    List(Vec<MetadataValue>),
    Dict(MetadataDict),
}

struct DocumentIndex {
    document_positions: Vec<(usize, usize)>,
    text_positions: Vec<(usize, usize)>,
    header_end: usize,
    text_leftovers: HashMap<usize, String>,
}

// Preallocated static constants for submission types
const SUBMISSION_TYPE_DASHED: &str = "dashed-default";
const SUBMISSION_TYPE_PRIVACY: &str = "tab-privacy";
const SUBMISSION_TYPE_DEFAULT: &str = "tab-default";

// Special tags as constants
const TAG_PDF: &str = "<PDF>";
const TAG_XBRL: &str = "<XBRL>";
const TAG_XML: &str = "<XML>";
const DOCUMENT_START: &str = "<DOCUMENT>";
const DOCUMENT_END: &str = "</DOCUMENT>";
const TEXT_START: &str = "<TEXT>";
const TEXT_END: &str = "</TEXT>";

// Detect submission type based on the first line - using &str instead of String
fn detect_submission_type(first_line: &str) -> Result<&'static str, SgmlParserError> {
    if first_line.starts_with("<SUBMISSION>") {
        Ok(SUBMISSION_TYPE_DASHED)
    } else if first_line.starts_with("-----BEGIN PRIVACY-ENHANCED MESSAGE-----") {
        Ok(SUBMISSION_TYPE_PRIVACY)
    } else if first_line.starts_with("<SEC-DOCUMENT>") {
        Ok(SUBMISSION_TYPE_DEFAULT)
    } else {
        Err(SgmlParserError::UnknownSubmissionType)
    }
}

// Detect if content is UU-encoded
#[inline]
fn detect_uu(first_line: &str) -> bool {
    first_line.trim().starts_with("begin")
}

// Use existing uudecode function but with optimization for the fallback
fn decode_uu(lines: &[String]) -> Vec<u8> {
    // First try the crate's implementation
    let content = lines.join("\n");

    if let Some((decoded, _)) = uudecode(&content) {
        return decoded;
    }

    // Fallback to our optimized implementation
    optimized_fallback_decode(lines)
}

fn optimized_fallback_decode(lines: &[String]) -> Vec<u8> {
    // Estimate capacity to avoid reallocations
    let mut result = Vec::with_capacity(lines.len() * 45); // Average estimate
    let mut in_data = false;
    let mut buffer = [0u8; 128]; // Preallocated buffer for line decoding

    for line in lines {
        let trimmed = line.trim();

        if !in_data {
            if trimmed.starts_with("begin") {
                in_data = true;
            }
            continue;
        }

        if trimmed.is_empty() || trimmed == "end" {
            break;
        }

        if trimmed.len() < 2 {
            continue;
        }

        let bytes = trimmed.as_bytes();
        let length = ((bytes[0] - b' ') & 0x3F) as usize;

        if length == 0 {
            continue;
        }

        // Optimized line decoding
        let mut out_idx = 0;
        let mut i = 1;

        while i + 3 < bytes.len() && out_idx < length {
            // Process 4 characters at once for 3 output bytes when possible
            if bytes[i] > b' '
                && bytes[i] <= b' ' + 64
                && bytes[i + 1] > b' '
                && bytes[i + 1] <= b' ' + 64
                && bytes[i + 2] > b' '
                && bytes[i + 2] <= b' ' + 64
                && bytes[i + 3] > b' '
                && bytes[i + 3] <= b' ' + 64
            {
                let val1 = (bytes[i] - b' ') & 0x3F;
                let val2 = (bytes[i + 1] - b' ') & 0x3F;
                let val3 = (bytes[i + 2] - b' ') & 0x3F;
                let val4 = (bytes[i + 3] - b' ') & 0x3F;

                if out_idx < length {
                    buffer[out_idx] = (val1 << 2) | (val2 >> 4);
                    out_idx += 1;
                }

                if out_idx < length {
                    buffer[out_idx] = ((val2 & 0xF) << 4) | (val3 >> 2);
                    out_idx += 1;
                }

                if out_idx < length {
                    buffer[out_idx] = ((val3 & 0x3) << 6) | val4;
                    out_idx += 1;
                }

                i += 4;
            } else {
                // Skip invalid characters
                i += 1;
            }
        }

        // Copy the valid bytes to the result
        result.extend_from_slice(&buffer[..out_idx]);
    }

    result
}

// Clean lines optimized for fewer allocations
fn clean_lines<'a>(lines: &'a [String]) -> Vec<&'a str> {
    if lines.is_empty() {
        return Vec::new();
    }

    // Find first non-empty line
    let mut start_idx = 0;
    while start_idx < lines.len() && lines[start_idx].trim().is_empty() {
        start_idx += 1;
    }

    if start_idx >= lines.len() {
        return Vec::new();
    }

    // Check if this is a special tag
    let first_line_trimmed = lines[start_idx].trim();

    if first_line_trimmed == TAG_PDF
        || first_line_trimmed == TAG_XBRL
        || first_line_trimmed == TAG_XML
    {
        let tag = &first_line_trimmed[1..first_line_trimmed.len() - 1];
        let end_tag = format!("</{}>", tag);

        // Find closing tag position
        let mut end_pos = lines.len();
        for (i, line) in lines[start_idx..].iter().enumerate().rev() {
            if line.trim() == end_tag {
                end_pos = start_idx + i;
                break;
            }
        }

        // Extract content between tags
        return lines[start_idx + 1..end_pos]
            .iter()
            .map(|s| s.as_str())
            .collect();
    }

    // Regular content
    lines[start_idx..].iter().map(|s| s.as_str()).collect()
}

// Process text content with optimizations
fn process_text_content(lines: &[String]) -> Result<Vec<u8>, SgmlParserError> {
    let cleaned_line_refs = clean_lines(lines);

    if cleaned_line_refs.is_empty() {
        return Ok(Vec::new());
    }

    if detect_uu(cleaned_line_refs[0]) {
        // Convert &str to String for the UU decoder
        let cleaned_lines: Vec<String> = cleaned_line_refs.iter().map(|&s| s.to_string()).collect();
        Ok(decode_uu(&cleaned_lines))
    } else {
        // Estimate capacity for the joined result
        let total_len =
            cleaned_line_refs.iter().map(|s| s.len()).sum::<usize>() + cleaned_line_refs.len();
        let mut result = String::with_capacity(total_len);

        for (i, &line) in cleaned_line_refs.iter().enumerate() {
            if i > 0 {
                result.push('\n');
            }
            result.push_str(line);
        }

        Ok(result.into_bytes())
    }
}

// Parse document metadata with fewer allocations
fn parse_document_metadata(lines: &[String]) -> MetadataDict {
    let mut metadata = HashMap::with_capacity(lines.len() / 2);
    let mut current_key = None;

    for line in lines {
        if line.starts_with('<') && line.contains('>') {
            let pos = line.find('>').unwrap();
            let key = line[1..pos].to_lowercase();
            let value = line[pos + 1..].trim().to_string();

            metadata.insert(key.clone(), MetadataValue::String(value));
            current_key = Some(key);
        } else if let Some(key) = &current_key {
            // Continuation of previous value
            if let Some(MetadataValue::String(value)) = metadata.get_mut(key) {
                value.reserve(line.len() + 1);
                value.push(' ');
                value.push_str(line.trim());
            }
        }
    }

    metadata
}

// Build document index with optimizations
fn build_document_index(lines: &[String]) -> DocumentIndex {
    let mut index = DocumentIndex {
        document_positions: Vec::with_capacity(10), // Reasonable initial capacity
        text_positions: Vec::with_capacity(10),
        header_end: 0,
        text_leftovers: HashMap::new(),
    };

    // Find first document to mark header end
    for (i, line) in lines.iter().enumerate() {
        if line == DOCUMENT_START {
            index.header_end = i;
            break;
        }
    }

    // Index all document and text positions
    let mut doc_start = usize::MAX;
    let mut text_start = usize::MAX;

    for (i, line) in lines.iter().enumerate() {
        if line == DOCUMENT_START {
            doc_start = i;
        } else if line == DOCUMENT_END {
            if doc_start != usize::MAX {
                index.document_positions.push((doc_start, i));
                doc_start = usize::MAX;
            }
        } else if line == TEXT_START {
            text_start = i;
        } else if line.contains(TEXT_END) {
            // Check if next non-empty line is </DOCUMENT>
            let mut next_line = None;
            for j in i + 1..lines.len() {
                let trimmed = lines[j].trim();
                if !trimmed.is_empty() {
                    next_line = Some(trimmed);
                    break;
                }
            }

            if text_start != usize::MAX && next_line == Some(DOCUMENT_END) {
                if line != TEXT_END {
                    if let Some(pos) = line.find(TEXT_END) {
                        index.text_leftovers.insert(i, line[..pos].to_string());
                    }
                }

                index.text_positions.push((text_start, i));
                text_start = usize::MAX;
            }
        }
    }

    index
}

// Optimized header metadata parsing
fn parse_header_metadata<'a>(lines: &'a [String], submission_type: &str) -> MetadataDict {
    let estimated_size = lines.len() / 3;
    let mut header_metadata = HashMap::with_capacity(estimated_size);

    match submission_type {
        SUBMISSION_TYPE_DASHED => {
            parse_dashed_metadata(lines, &mut header_metadata);
        }
        SUBMISSION_TYPE_PRIVACY | SUBMISSION_TYPE_DEFAULT => {
            parse_tab_metadata(lines, &mut header_metadata, submission_type);
        }
        _ => {}
    }

    header_metadata
}

// Split the metadata parsing for better organization
fn parse_dashed_metadata(lines: &[String], header_metadata: &mut MetadataDict) {
    #[derive(Clone)]
    struct StackItem {
        tag: String,
        dict: MetadataDict,
    }

    let mut stack: Vec<StackItem> = Vec::with_capacity(8);

    for (i, line) in lines.iter().enumerate() {
        if !line.contains('>') {
            continue;
        }

        if let Some(pos) = line.find('>') {
            let mut tag = (&line[1..pos]).to_lowercase();
            let text = line[pos + 1..].trim();

            // Handle closing tags
            if tag.starts_with('/') {
                tag = tag[1..].to_string(); // Remove the '/'

                // Pop from stack if matching tag
                if let Some(item) = stack.pop() {
                    if item.tag == tag {
                        if stack.is_empty() {
                            header_metadata.insert(tag, MetadataValue::Dict(item.dict));
                        } else if let Some(parent) = stack.last_mut() {
                            parent.dict.insert(tag, MetadataValue::Dict(item.dict));
                        }
                    } else {
                        // Tags don't match, put it back
                        stack.push(item);
                    }
                }
                continue;
            }

            // Look ahead to check if this tag has a closing tag
            let has_closing_tag = lines[i + 1..].iter().any(|l| {
                let l_lower = l.to_lowercase();
                l_lower.contains(&format!("</{}>", tag))
            });

            if has_closing_tag {
                // This tag has a closing tag, push to stack
                stack.push(StackItem {
                    tag: tag.clone(),
                    dict: HashMap::with_capacity(8),
                });
            } else if !text.is_empty() {
                // This is a leaf tag with text content
                if let Some(item) = stack.last_mut() {
                    insert_or_append(&mut item.dict, tag, MetadataValue::String(text.to_string()));
                } else {
                    insert_or_append(
                        header_metadata,
                        tag,
                        MetadataValue::String(text.to_string()),
                    );
                }
            }
        }
    }

    // Process any remaining items in stack
    while let Some(item) = stack.pop() {
        if stack.is_empty() {
            header_metadata.insert(item.tag, MetadataValue::Dict(item.dict));
        } else if let Some(parent) = stack.last_mut() {
            parent.dict.insert(item.tag, MetadataValue::Dict(item.dict));
        }
    }
}

fn parse_tab_metadata(lines: &[String], header_metadata: &mut MetadataDict, submission_type: &str) {
    struct IndentedItem {
        indent: usize,
        tag: String,
        dict: MetadataDict,
    }

    let mut stack: Vec<IndentedItem> = Vec::with_capacity(8);
    let mut lines_to_process = lines;
    let mut privacy_msg = String::new();

    // Handle privacy-enhanced message specially
    if submission_type == SUBMISSION_TYPE_PRIVACY {
        let mut found_start = false;
        let mut skip_lines = 0;

        for (i, line) in lines.iter().enumerate() {
            if line.trim() == "-----BEGIN PRIVACY-ENHANCED MESSAGE-----" {
                found_start = true;
                skip_lines = i + 1;
                continue;
            }

            if found_start {
                let trimmed = line.trim();
                if trimmed.is_empty()
                    || (line.contains('<')
                        && line
                            .find('<')
                            .map(|pos| line[pos + 1..].chars().any(|c| c.is_uppercase()))
                            .unwrap_or(false))
                {
                    break;
                }

                if !privacy_msg.is_empty() {
                    privacy_msg.push('\n');
                }
                privacy_msg.push_str(trimmed);
                skip_lines += 1;
            }
        }

        if !privacy_msg.is_empty() {
            header_metadata.insert(
                "privacy-enhanced-message".to_string(),
                MetadataValue::String(privacy_msg),
            );

            if skip_lines < lines.len() {
                lines_to_process = &lines[skip_lines..];
            } else {
                lines_to_process = &[];
            }
        }
    }

    for line in lines_to_process {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let indent = line.len() - line.trim_start().len();

        // Parse tag and text
        let (tag, text) = if let Some(pos) = line.find('>') {
            let tag_part = &line[1..pos];
            if tag_part.starts_with('/') {
                continue; // Skip closing tags
            }
            (
                tag_part.trim().to_lowercase(),
                line[pos + 1..].trim().to_string(),
            )
        } else if let Some(pos) = line.find(':') {
            (
                line[..pos].trim().to_lowercase(),
                line[pos + 1..].trim().to_string(),
            )
        } else {
            continue;
        };

        // Remove entries from stack with greater or equal indent
        while !stack.is_empty() && stack.last().unwrap().indent >= indent {
            let item = stack.pop().unwrap();

            if stack.is_empty() {
                header_metadata.insert(item.tag, MetadataValue::Dict(item.dict));
            } else {
                stack
                    .last_mut()
                    .unwrap()
                    .dict
                    .insert(item.tag, MetadataValue::Dict(item.dict));
            }
        }

        if !text.is_empty() {
            // Text content
            if let Some(item) = stack.last_mut() {
                insert_or_append(&mut item.dict, tag.clone(), MetadataValue::String(text));
            } else {
                insert_or_append(header_metadata, tag.clone(), MetadataValue::String(text));
            }
        } else {
            // No text, this is a container
            // Remove entries with same indent
            while !stack.is_empty() && stack.last().unwrap().indent == indent {
                let item = stack.pop().unwrap();

                if stack.is_empty() {
                    header_metadata.insert(item.tag, MetadataValue::Dict(item.dict));
                } else {
                    stack
                        .last_mut()
                        .unwrap()
                        .dict
                        .insert(item.tag, MetadataValue::Dict(item.dict));
                }
            }

            // Push new container
            stack.push(IndentedItem {
                indent,
                tag: tag.clone(),
                dict: HashMap::with_capacity(8),
            });
        }
    }

    // Process any remaining items in stack
    while let Some(item) = stack.pop() {
        if stack.is_empty() {
            header_metadata.insert(item.tag, MetadataValue::Dict(item.dict));
        } else {
            stack
                .last_mut()
                .unwrap()
                .dict
                .insert(item.tag, MetadataValue::Dict(item.dict));
        }
    }
}

// Helper function to insert a value or append to a list if key exists
fn insert_or_append(dict: &mut MetadataDict, key: String, value: MetadataValue) {
    if let Some(existing) = dict.get_mut(&key) {
        match existing {
            MetadataValue::List(list) => {
                list.push(value);
            }
            _ => {
                let old_value = existing.clone();
                *existing = MetadataValue::List(vec![old_value, value]);
            }
        }
    } else {
        dict.insert(key, value);
    }
}

/// Parse SGML submission - main entry point
pub fn parse_sgml_submission(
    content: &str,
    filepath: Option<&str>,
) -> Result<(MetadataDict, Vec<Vec<u8>>), SgmlParserError> {
    let content_str = if let Some(path) = filepath {
        fs::read_to_string(path)?
    } else {
        content.to_string()
    };

    let lines: Vec<String> = content_str.lines().map(String::from).collect();

    if lines.is_empty() {
        return Ok((MetadataDict::new(), Vec::new()));
    }

    // Detect submission type
    let submission_type = detect_submission_type(&lines[0])?;

    // Get document structure index
    let doc_index = build_document_index(&lines);

    // Parse header metadata
    let header_lines = &lines[..doc_index.header_end];
    let mut metadata = parse_header_metadata(header_lines, submission_type);

    // Process documents using indexed positions
    let estimated_docs = doc_index.document_positions.len();
    let mut documents = Vec::with_capacity(estimated_docs);
    let mut documents_metadata = Vec::with_capacity(estimated_docs);

    for (doc_start, doc_end) in &doc_index.document_positions {
        // Find corresponding text section for this document
        let mut text_start = None;
        let mut text_end = None;

        for (start, end) in &doc_index.text_positions {
            if start > doc_start && end < doc_end {
                text_start = Some(*start);
                text_end = Some(*end);
                break;
            }
        }

        if let (Some(start), Some(end)) = (text_start, text_end) {
            // Extract document metadata
            let doc_metadata = parse_document_metadata(&lines[doc_start + 1..start]);
            documents_metadata.push(doc_metadata);

            // Process text contents
            let mut text_lines: Vec<String>;

            // Optimization: If no leftovers, use slice directly
            if doc_index.text_leftovers.contains_key(&end) {
                text_lines = lines[start + 1..end].to_vec();
                text_lines.push(doc_index.text_leftovers[&end].clone());
            } else {
                text_lines = lines[start + 1..=end].to_vec();
            }

            // Process content and add to documents list
            let content_bytes = process_text_content(&text_lines)?;
            documents.push(content_bytes);
        }
    }

    // Add documents metadata to the metadata dictionary
    if !documents_metadata.is_empty() {
        metadata.insert(
            "documents".to_string(),
            MetadataValue::List(
                documents_metadata
                    .into_iter()
                    .map(MetadataValue::Dict)
                    .collect(),
            ),
        );
    }

    Ok((metadata, documents))
}
