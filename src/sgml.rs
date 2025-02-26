use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::io::{ensure_output_dir, prepare_documents, write_documents, write_metadata};
use crate::types::{DocumentIndex, MetadataDict, MetadataValue, ParseError, SubmissionType};
use crate::utils::process_text_content;

pub fn detect_submission_type(first_line: &str) -> Result<SubmissionType, ParseError> {
    if first_line.starts_with("<SUBMISSION>") {
        Ok(SubmissionType::DashedDefault)
    } else if first_line.starts_with("-----BEGIN PRIVACY-ENHANCED MESSAGE-----") {
        Ok(SubmissionType::TabPrivacy)
    } else if first_line.starts_with("<SEC-DOCUMENT>") {
        Ok(SubmissionType::TabDefault)
    } else {
        Err(ParseError::UnknownSubmissionType(first_line.to_string()))
    }
}

pub fn build_document_index(lines: &[String]) -> DocumentIndex {
    let mut index = DocumentIndex::new();
    let mut doc_start: isize = -1;
    let mut text_start: isize = -1;

    for (i, line) in lines.iter().enumerate() {
        if line == "<DOCUMENT>" {
            if doc_start < 0 && index.header_end == 0 {
                index.header_end = i;
            }
            doc_start = i as isize;
        } else if line == "</DOCUMENT>" {
            if doc_start >= 0 {
                index.document_positions.push((doc_start as usize, i));
                doc_start = -1;
            }
        } else if line == "<TEXT>" {
            text_start = i as isize;
        } else if line.contains("</TEXT>") {
            let next_line = lines[i + 1..]
                .iter()
                .find(|line| !line.trim().is_empty())
                .map(|line| line.trim());

            if next_line == Some("</DOCUMENT>") && text_start >= 0 {
                if line != "</TEXT>" {
                    let parts: Vec<&str> = line.split("</TEXT>").collect();
                    if !parts.is_empty() {
                        index.text_leftovers.insert(i, parts[0].to_string());
                    }
                }
                index.text_positions.push((text_start as usize, i));
                text_start = -1;
            }
        }
    }

    index
}

fn parse_document_metadata(lines: &[String]) -> MetadataDict {
    let mut metadata = HashMap::new();
    let mut current_key = None;

    for line in lines {
        if line.starts_with('<') && line.contains('>') {
            let parts: Vec<&str> = line.split('>').collect();
            if parts.len() < 2 {
                continue;
            }

            let key = parts[0][1..].to_lowercase();
            let value = parts[1].trim().to_string();

            current_key = Some(key.clone());
            metadata.insert(key, MetadataValue::Text(value));
        } else if let Some(key) = &current_key {
            if let Some(MetadataValue::Text(text)) = metadata.get_mut(key) {
                *text = format!("{} {}", text, line.trim());
            }
        }
    }

    metadata
}

fn parse_header_metadata(lines: &[String], submission_type: &SubmissionType) -> MetadataDict {
    match submission_type {
        SubmissionType::DashedDefault => parse_dashed_default_header(lines),
        SubmissionType::TabPrivacy | SubmissionType::TabDefault => {
            parse_tab_header(lines, submission_type)
        }
    }
}

fn parse_dashed_default_header(lines: &[String]) -> MetadataDict {
    let mut header_metadata = HashMap::new();
    let mut tag_stack: Vec<String> = Vec::new();
    let mut dict_stack: Vec<MetadataDict> = vec![header_metadata.clone()];

    for (i, line) in lines.iter().enumerate() {
        if !line.contains('>') {
            continue;
        }

        let parts: Vec<&str> = line.split('>').collect();
        if parts.is_empty() || !parts[0].starts_with('<') {
            continue;
        }

        let tag = parts[0][1..].to_lowercase();
        let text = parts.get(1).map_or("", |s| s.trim()).to_string();

        // Handle closing tags
        if tag.starts_with('/') {
            let tag_name = tag[1..].to_string();

            if tag_stack.last().map_or(false, |t| t == &tag_name) {
                tag_stack.pop();
                dict_stack.pop();
            }
            continue;
        }

        // Look ahead to check if this tag has a closing tag
        let has_closing_tag = lines[i + 1..]
            .iter()
            .any(|l| l.trim().to_lowercase().starts_with(&format!("</{}>", tag)));

        let current_dict = dict_stack.last_mut().unwrap();

        if has_closing_tag {
            // Create new nested dict
            let nested_dict = HashMap::new();

            // Insert into current dict
            match current_dict.get(&tag) {
                Some(MetadataValue::Dict(existing)) => {
                    let mut list = vec![MetadataValue::Dict(existing.clone())];
                    list.push(MetadataValue::Dict(nested_dict.clone()));
                    current_dict.insert(tag.clone(), MetadataValue::List(list));
                }
                Some(MetadataValue::List(list)) => {
                    let mut new_list = list.clone();
                    new_list.push(MetadataValue::Dict(nested_dict.clone()));
                    current_dict.insert(tag.clone(), MetadataValue::List(new_list));
                }
                Some(MetadataValue::Text(_)) => {
                    let existing = current_dict.remove(&tag).unwrap();
                    let list = vec![existing, MetadataValue::Dict(nested_dict.clone())];
                    current_dict.insert(tag.clone(), MetadataValue::List(list));
                }
                None => {
                    current_dict.insert(tag.clone(), MetadataValue::Dict(nested_dict.clone()));
                }
            }

            // Push new context to stacks
            tag_stack.push(tag);
            dict_stack.push(nested_dict);
        } else if !text.is_empty() {
            // Add text value
            match current_dict.get(&tag) {
                Some(MetadataValue::Text(existing)) => {
                    let list = vec![
                        MetadataValue::Text(existing.clone()),
                        MetadataValue::Text(text),
                    ];
                    current_dict.insert(tag.clone(), MetadataValue::List(list));
                }
                Some(MetadataValue::List(list)) => {
                    let mut new_list = list.clone();
                    new_list.push(MetadataValue::Text(text));
                    current_dict.insert(tag.clone(), MetadataValue::List(new_list));
                }
                Some(MetadataValue::Dict(_)) => {
                    let existing = current_dict.remove(&tag).unwrap();
                    let list = vec![existing, MetadataValue::Text(text)];
                    current_dict.insert(tag.clone(), MetadataValue::List(list));
                }
                None => {
                    current_dict.insert(tag.clone(), MetadataValue::Text(text));
                }
            }
        }
    }

    dict_stack[0].clone()
}

fn parse_tab_header(lines: &[String], submission_type: &SubmissionType) -> MetadataDict {
    let mut header_metadata = HashMap::new();

    // Handle privacy-enhanced message if needed
    if *submission_type == SubmissionType::TabPrivacy {
        let mut privacy_msg = Vec::new();
        let mut i = 0;

        while i < lines.len() {
            if lines[i].trim() == "-----BEGIN PRIVACY-ENHANCED MESSAGE-----" {
                i += 1;
                while i < lines.len() {
                    let line = lines[i].trim();
                    if line.is_empty()
                        || (line.contains('<') && line.chars().any(|c| c.is_uppercase()))
                    {
                        break;
                    }
                    privacy_msg.push(line);
                    i += 1;
                }

                header_metadata.insert(
                    "privacy-enhanced-message".to_string(),
                    MetadataValue::Text(privacy_msg.join("\n")),
                );
                break;
            }
            i += 1;
        }
    }

    // Use indentation to track nesting
    let mut indent_stack: Vec<usize> = vec![0];
    let mut dict_stack: Vec<MetadataDict> = vec![header_metadata.clone()];

    for line in lines {
        if line.trim().is_empty() {
            continue;
        }

        let indent = line.len() - line.trim_start().len();

        // Parse tag and text
        let (tag, text) = if line.contains('>') {
            let parts: Vec<&str> = line.split('>').collect();
            if parts.len() < 2 || !parts[0].starts_with('<') {
                continue;
            }

            let tag = parts[0][1..].to_lowercase().trim().to_string();
            if tag.starts_with('/') {
                continue;
            }

            (tag, parts[1].trim().to_string())
        } else if line.contains(':') {
            let parts: Vec<&str> = line.splitn(2, ':').collect();
            if parts.len() < 2 {
                continue;
            }

            (parts[0].trim().to_lowercase(), parts[1].trim().to_string())
        } else {
            continue;
        };

        // Find appropriate parent based on indentation
        while indent_stack.len() > 1 && indent_stack.last().unwrap() >= &indent {
            indent_stack.pop();
            dict_stack.pop();
        }

        let current_dict = dict_stack.last_mut().unwrap();

        if !text.is_empty() {
            // Add text value to current dict
            match current_dict.get(&tag) {
                Some(MetadataValue::Text(existing)) => {
                    let list = vec![
                        MetadataValue::Text(existing.clone()),
                        MetadataValue::Text(text),
                    ];
                    current_dict.insert(tag.clone(), MetadataValue::List(list));
                }
                Some(MetadataValue::List(list)) => {
                    let mut new_list = list.clone();
                    new_list.push(MetadataValue::Text(text));
                    current_dict.insert(tag.clone(), MetadataValue::List(new_list));
                }
                Some(MetadataValue::Dict(_)) => {
                    let existing = current_dict.remove(&tag).unwrap();
                    let list = vec![existing, MetadataValue::Text(text)];
                    current_dict.insert(tag.clone(), MetadataValue::List(list));
                }
                None => {
                    current_dict.insert(tag.clone(), MetadataValue::Text(text));
                }
            }
        } else {
            // Create nested dict with ownership transfer
            let nested_dict = HashMap::new();

            // Insert into current dict
            match current_dict.get(&tag) {
                Some(MetadataValue::Dict(existing)) => {
                    let mut list = vec![MetadataValue::Dict(existing.clone())];
                    list.push(MetadataValue::Dict(nested_dict.clone()));
                    current_dict.insert(tag.clone(), MetadataValue::List(list));
                }
                Some(MetadataValue::List(list)) => {
                    let mut new_list = list.clone();
                    new_list.push(MetadataValue::Dict(nested_dict.clone()));
                    current_dict.insert(tag.clone(), MetadataValue::List(new_list));
                }
                Some(MetadataValue::Text(_)) => {
                    let existing = current_dict.remove(&tag).unwrap();
                    let list = vec![existing, MetadataValue::Dict(nested_dict.clone())];
                    current_dict.insert(tag.clone(), MetadataValue::List(list));
                }
                None => {
                    current_dict.insert(tag.clone(), MetadataValue::Dict(nested_dict.clone()));
                }
            }

            // Push new context to stacks
            indent_stack.push(indent);
            dict_stack.push(nested_dict);
        }
    }

    dict_stack[0].clone()
}

pub fn parse_sgml_into_memory(
    content: Option<String>,
    filepath: Option<&Path>,
) -> Result<(MetadataDict, Vec<Vec<u8>>), ParseError> {
    if filepath.is_none() && content.is_none() {
        return Err(ParseError::NoInput);
    }

    // Read content if not provided
    let content = match content {
        Some(c) => c,
        None => fs::read_to_string(filepath.unwrap()).map_err(|e| ParseError::Io(e))?,
    };

    let lines: Vec<String> = content.lines().map(String::from).collect();

    if lines.is_empty() {
        return Err(ParseError::InvalidContent("Empty content".to_string()));
    }

    // Detect submission type
    let submission_type = detect_submission_type(&lines[0])?;

    // Get document structure index
    let doc_index = build_document_index(&lines);

    // Parse header metadata
    let header_lines = &lines[..doc_index.header_end];
    let mut metadata = parse_header_metadata(header_lines, &submission_type);

    // Process documents using indexed positions
    let mut documents = Vec::new();
    let mut doc_metadata_list = Vec::new();

    for &(doc_start, doc_end) in &doc_index.document_positions {
        // Find corresponding text section for this document
        let mut text_range = None;

        for &(start, end) in &doc_index.text_positions {
            if start > doc_start && end < doc_end {
                text_range = Some((start, end));
                break;
            }
        }

        if let Some((text_start, text_end)) = text_range {
            // Extract document metadata
            let doc_metadata = parse_document_metadata(&lines[doc_start + 1..text_start]);
            doc_metadata_list.push(MetadataValue::Dict(doc_metadata));

            // Process text contents
            let mut text_lines = lines[text_start + 1..text_end].to_vec();

            // If there's leftover content at the end
            if let Some(leftover) = doc_index.text_leftovers.get(&text_end) {
                text_lines.push(leftover.clone());
            }

            // Process content and add to documents list
            let content_bytes = process_text_content(&text_lines);
            documents.push(content_bytes);
        }
    }

    // Add document metadata to the metadata dictionary
    metadata.insert(
        "documents".to_string(),
        MetadataValue::List(doc_metadata_list),
    );

    Ok((metadata, documents))
}

pub fn parse_sgml_submission(
    content: Option<String>,
    filepath: Option<&Path>,
    output_dir: &Path,
) -> Result<(), ParseError> {
    // Parse SGML into memory
    let (metadata, documents) = parse_sgml_into_memory(content, filepath)?;

    // Ensure output directory exists
    ensure_output_dir(output_dir)?;

    // Write metadata to JSON file
    write_metadata(&metadata, output_dir)?;

    // Prepare and write documents
    let doc_infos = prepare_documents(documents, &metadata);
    write_documents(doc_infos, output_dir)?;

    Ok(())
}
