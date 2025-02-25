use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, Read};
use std::iter::Iterator;
use std::path::Path;
use std::str;

// Import our UU decoder
use crate::uu_decode;

type MetadataDict = HashMap<String, Value>;

// Constants for submission types
lazy_static! {
    static ref SUBMISSION_TYPES: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("<SUBMISSION>", "dashed-default");
        m.insert("-----BEGIN PRIVACY-ENHANCED MESSAGE-----", "tab-privacy");
        m.insert("<SEC-DOCUMENT>", "tab-default");
        m
    };
    static ref SPECIAL_TAGS: HashSet<&'static str> = {
        let mut s = HashSet::new();
        s.insert("<PDF>");
        s.insert("<XBRL>");
        s.insert("<XML>");
        s
    };
}

#[derive(Debug)]
struct DocumentIndex {
    document_positions: Vec<(usize, usize)>,
    text_positions: Vec<(usize, usize)>,
    header_end: usize,
    text_leftovers: HashMap<usize, String>,
}

impl DocumentIndex {
    fn new() -> Self {
        DocumentIndex {
            document_positions: Vec::new(),
            text_positions: Vec::new(),
            header_end: 0,
            text_leftovers: HashMap::new(),
        }
    }
}

fn detect_submission_type(first_line: &str) -> Result<&'static str, String> {
    for (marker, submission_type) in SUBMISSION_TYPES.iter() {
        if first_line.starts_with(marker) {
            return Ok(*submission_type);
        }
    }
    Err("Unknown submission type".to_string())
}

pub fn determine_file_extension(doc_meta: &Value) -> String {
    if let Value::Object(obj) = doc_meta {
        // Check for document type in metadata
        if let Some(Value::String(doc_type)) = obj.get("type") {
            match doc_type.to_lowercase().as_str() {
                "10-k" | "10-q" | "8-k" => return "txt".to_string(),
                "html" | "htm" => return "html".to_string(),
                "xml" => return "xml".to_string(),
                "pdf" => return "pdf".to_string(),
                "xbrl" => return "xbrl".to_string(),
                "graphic" | "jpg" | "jpeg" => return "jpg".to_string(),
                "png" => return "png".to_string(),
                "gif" => return "gif".to_string(),
                // Add more mappings as needed
                _ => {}
            }
        }

        // Check filename if present
        if let Some(Value::String(filename)) = obj.get("filename") {
            if let Some(ext) = Path::new(filename).extension() {
                if let Some(ext_str) = ext.to_str() {
                    return ext_str.to_string();
                }
            }
        }

        // Check document format
        if let Some(Value::String(format)) = obj.get("format") {
            match format.to_lowercase().as_str() {
                "html" | "htm" => return "html".to_string(),
                "pdf" => return "pdf".to_string(),
                "xml" => return "xml".to_string(),
                "text" | "ascii" => return "txt".to_string(),
                "jpg" | "jpeg" => return "jpg".to_string(),
                "png" => return "png".to_string(),
                "gif" => return "gif".to_string(),
                // Add more mappings as needed
                _ => {}
            }
        }
    }

    // Default extension if nothing matches
    "txt".to_string()
}

fn parse_header_metadata(lines: &[String], submission_type: &str) -> MetadataDict {
    let mut header_metadata: MetadataDict = HashMap::new();

    if submission_type == "dashed-default" {
        // We'll handle dashed-default in a way that simulates the recursive structure
        // but works with Rust's ownership model
        let mut tag_stack: Vec<String> = Vec::new();
        let mut dict_stack: Vec<MetadataDict> = vec![HashMap::new()];

        for line in lines {
            if let Some(pos) = line.find('>') {
                let tag = line[1..pos].to_lowercase();
                let text = line[pos + 1..].trim().to_string();

                // Handle closing tags
                if tag.starts_with('/') {
                    let close_tag = tag[1..].to_string();
                    if !tag_stack.is_empty() && tag_stack.last().unwrap() == &close_tag {
                        let finished_dict = dict_stack.pop().unwrap();
                        let finished_tag = tag_stack.pop().unwrap();

                        if !dict_stack.is_empty() {
                            let parent_dict = dict_stack.last_mut().unwrap();

                            if parent_dict.contains_key(&finished_tag) {
                                // Convert to array if already exists
                                match parent_dict.get_mut(&finished_tag) {
                                    Some(Value::Array(arr)) => {
                                        arr.push(Value::Object(
                                            finished_dict
                                                .iter()
                                                .map(|(k, v)| (k.clone(), v.clone()))
                                                .collect(),
                                        ));
                                    }
                                    Some(existing) => {
                                        let existing_value = existing.clone();
                                        let mut new_arr = Vec::new();
                                        new_arr.push(existing_value);
                                        new_arr.push(Value::Object(
                                            finished_dict
                                                .iter()
                                                .map(|(k, v)| (k.clone(), v.clone()))
                                                .collect(),
                                        ));
                                        *existing = Value::Array(new_arr);
                                    }
                                    None => unreachable!(),
                                }
                            } else {
                                parent_dict.insert(
                                    finished_tag,
                                    Value::Object(
                                        finished_dict
                                            .iter()
                                            .map(|(k, v)| (k.clone(), v.clone()))
                                            .collect(),
                                    ),
                                );
                            }
                        }
                    }
                    continue;
                }

                // Look ahead for closing tag
                let has_closing_tag = lines
                    .iter()
                    .any(|l| l.trim().to_lowercase().starts_with(&format!("</{}>", tag)));

                if has_closing_tag {
                    // Start a new nested dict
                    tag_stack.push(tag);
                    dict_stack.push(HashMap::new());
                } else if !text.is_empty() {
                    // Add text value to current dict
                    let current_dict = dict_stack.last_mut().unwrap();

                    if current_dict.contains_key(&tag) {
                        // Convert to array if already exists
                        match current_dict.get_mut(&tag) {
                            Some(Value::Array(arr)) => {
                                arr.push(Value::String(text));
                            }
                            Some(existing) => {
                                let existing_value = existing.clone();
                                let mut new_arr = Vec::new();
                                new_arr.push(existing_value);
                                new_arr.push(Value::String(text));
                                *existing = Value::Array(new_arr);
                            }
                            None => unreachable!(),
                        }
                    } else {
                        current_dict.insert(tag, Value::String(text));
                    }
                }
            }
        }

        // Merge all remaining dicts into the header metadata
        while !dict_stack.is_empty() {
            let dict = dict_stack.pop().unwrap();
            if dict_stack.is_empty() {
                // This is the root dictionary
                header_metadata = dict;
            } else if !tag_stack.is_empty() {
                let tag = tag_stack.pop().unwrap();
                let parent_dict = dict_stack.last_mut().unwrap();

                if parent_dict.contains_key(&tag) {
                    // Convert to array if already exists
                    match parent_dict.get_mut(&tag) {
                        Some(Value::Array(arr)) => {
                            arr.push(Value::Object(
                                dict.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                            ));
                        }
                        Some(existing) => {
                            let existing_value = existing.clone();
                            let mut new_arr = Vec::new();
                            new_arr.push(existing_value);
                            new_arr.push(Value::Object(
                                dict.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                            ));
                            *existing = Value::Array(new_arr);
                        }
                        None => unreachable!(),
                    }
                } else {
                    parent_dict.insert(
                        tag,
                        Value::Object(dict.iter().map(|(k, v)| (k.clone(), v.clone())).collect()),
                    );
                }
            }
        }
    } else {
        // tab-default or tab-privacy
        if submission_type == "tab-privacy" {
            // Handle privacy message
            let mut privacy_msg = Vec::new();
            let mut i = 0;

            while i < lines.len() {
                if lines[i].trim() == "-----BEGIN PRIVACY-ENHANCED MESSAGE-----" {
                    i += 1;
                    while i < lines.len() {
                        let line = &lines[i];
                        if line.trim().is_empty()
                            || (line.contains('<')
                                && line[line.find('<').unwrap() + 1..]
                                    .chars()
                                    .any(|c| c.is_uppercase()))
                        {
                            break;
                        }
                        privacy_msg.push(line.trim().to_string());
                        i += 1;
                    }

                    header_metadata.insert(
                        "privacy-enhanced-message".to_string(),
                        Value::String(privacy_msg.join("\n")),
                    );
                    break;
                }
                i += 1;
            }
        }

        // Process indented structure - using a stack-based approach
        let mut stack: Vec<(usize, String, MetadataDict)> = Vec::new(); // (indent, tag, dict)
        stack.push((0, "root".to_string(), HashMap::new()));

        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let indent = line.len() - line.trim_start().len();
            let mut tag = String::new();
            let mut text = String::new();

            // Parse the line
            if let Some(pos) = line.find('>') {
                // XML-style tag
                if line.starts_with('<') {
                    tag = line[1..pos].trim().to_lowercase();
                    if tag.starts_with('/') {
                        continue; // Skip closing tags
                    }
                    text = line[pos + 1..].trim().to_string();
                }
            } else if let Some(pos) = line.find(':') {
                // Key-value style
                tag = line[..pos].trim().to_lowercase();
                text = line[pos + 1..].trim().to_string();
            }

            // Pop stack if needed based on indentation
            while stack.len() > 1 && stack.last().unwrap().0 >= indent {
                let (_, tag_name, dict) = stack.pop().unwrap();
                let (_, _, parent_dict) = stack.last_mut().unwrap();

                if parent_dict.contains_key(&tag_name) {
                    // Convert to array if already exists
                    match parent_dict.get_mut(&tag_name) {
                        Some(Value::Array(arr)) => {
                            arr.push(Value::Object(dict.into_iter().collect()));
                        }
                        Some(existing) => {
                            let existing_value = existing.clone();
                            let mut new_arr = Vec::new();
                            new_arr.push(existing_value);
                            new_arr.push(Value::Object(dict.into_iter().collect()));
                            *existing = Value::Array(new_arr);
                        }
                        None => unreachable!(),
                    }
                } else {
                    parent_dict.insert(tag_name, Value::Object(dict.into_iter().collect()));
                }
            }

            if !text.is_empty() {
                // Add text value
                let (_, _, current_dict) = stack.last_mut().unwrap();

                if current_dict.contains_key(&tag) {
                    // Convert to array if already exists
                    match current_dict.get_mut(&tag) {
                        Some(Value::Array(arr)) => {
                            arr.push(Value::String(text));
                        }
                        Some(existing) => {
                            let existing_value = existing.clone();
                            let mut new_arr = Vec::new();
                            new_arr.push(existing_value);
                            new_arr.push(Value::String(text));
                            *existing = Value::Array(new_arr);
                        }
                        None => unreachable!(),
                    }
                } else {
                    current_dict.insert(tag, Value::String(text));
                }
            } else if !tag.is_empty() {
                // Start a new nested dict
                stack.push((indent, tag, HashMap::new()));
            }
        }

        // Merge all remaining dicts
        while stack.len() > 1 {
            let (_, tag_name, dict) = stack.pop().unwrap();
            let (_, _, parent_dict) = stack.last_mut().unwrap();

            if parent_dict.contains_key(&tag_name) {
                // Convert to array if already exists
                match parent_dict.get_mut(&tag_name) {
                    Some(Value::Array(arr)) => {
                        arr.push(Value::Object(dict.into_iter().collect()));
                    }
                    Some(existing) => {
                        let existing_value = existing.clone();
                        let mut new_arr = Vec::new();
                        new_arr.push(existing_value);
                        new_arr.push(Value::Object(dict.into_iter().collect()));
                        *existing = Value::Array(new_arr);
                    }
                    None => unreachable!(),
                }
            } else {
                parent_dict.insert(tag_name, Value::Object(dict.into_iter().collect()));
            }
        }

        if !stack.is_empty() {
            let (_, _, root_dict) = stack.pop().unwrap();
            header_metadata = root_dict;
        }
    }

    header_metadata
}

fn detect_uu(first_line: &str) -> bool {
    first_line.trim().starts_with("begin")
}

fn clean_lines(lines: &[String]) -> Vec<String> {
    if lines.is_empty() {
        return Vec::new();
    }

    // Skip empty lines at the beginning
    let mut first_non_empty = 0;
    while first_non_empty < lines.len() && lines[first_non_empty].trim().is_empty() {
        first_non_empty += 1;
    }

    if first_non_empty >= lines.len() {
        return Vec::new();
    }

    let first_line = lines[first_non_empty].trim();

    if SPECIAL_TAGS.contains(first_line) {
        let tag = &first_line[1..first_line.len() - 1];
        let end_tag = format!("</{}>", tag);

        // Find closing tag position
        let mut end_pos = lines.len();
        for (i, line) in lines.iter().enumerate().rev() {
            if line.trim() == end_tag {
                end_pos = i;
                break;
            }
        }

        return lines[first_non_empty + 1..end_pos].to_vec();
    }

    lines[first_non_empty..].to_vec()
}

fn process_text_content(lines: &[String]) -> Vec<u8> {
    let cleaned_lines = clean_lines(lines);

    if cleaned_lines.is_empty() {
        return Vec::new();
    }

    if detect_uu(&cleaned_lines[0]) {
        // Use our UU decoder
        return uu_decode::decode(&cleaned_lines);
    } else {
        // Regular text content
        return cleaned_lines.join("\n").into_bytes();
    }
}

fn parse_document_metadata(lines: &[String]) -> MetadataDict {
    let mut metadata = HashMap::new();
    let mut current_key: Option<String> = None;

    for line in lines {
        if line.starts_with('<') {
            if let Some(pos) = line.find('>') {
                let key = line[1..pos].to_lowercase();
                let value = line[pos + 1..].trim();
                metadata.insert(key.clone(), Value::String(value.to_string()));
                current_key = Some(key);
            }
        } else if let Some(ref key) = current_key {
            // Continuation of previous value
            if let Some(Value::String(ref mut val)) = metadata.get_mut(key) {
                *val = format!("{} {}", val, line.trim());
            }
        }
    }

    metadata
}

fn build_document_index(lines: &[String]) -> DocumentIndex {
    let mut index = DocumentIndex::new();
    let mut doc_start: isize = -1;
    let mut text_start: isize = -1;

    // Find first document to mark header end
    for (i, line) in lines.iter().enumerate() {
        if line == "<DOCUMENT>" {
            index.header_end = i;
            break;
        }
    }

    // Index all document and text positions
    for (i, line) in lines.iter().enumerate() {
        if line == "<DOCUMENT>" {
            doc_start = i as isize;
        } else if line == "</DOCUMENT>" {
            if doc_start >= 0 {
                index.document_positions.push((doc_start as usize, i));
                doc_start = -1;
            }
        } else if line == "<TEXT>" {
            text_start = i as isize;
        } else if line.contains("</TEXT>") {
            // Check if text_start is valid
            if text_start >= 0 {
                // Handle case where </TEXT> is at the end of line but not the entire line
                if line != "</TEXT>" {
                    let parts: Vec<&str> = line.split("</TEXT>").collect();
                    index.text_leftovers.insert(i, parts[0].to_string());
                }
                index.text_positions.push((text_start as usize, i));
                text_start = -1;
            }
        }
    }

    index
}

pub fn parse_sgml_submission_into_memory(
    content: Option<String>,
    filepath: Option<&Path>,
) -> Result<(MetadataDict, Vec<Vec<u8>>), String> {
    if content.is_none() && filepath.is_none() {
        return Err("Either filepath or content must be provided".to_string());
    }

    // Read content if not provided
    let content = match content {
        Some(c) => c,
        None => {
            let mut file =
                File::open(filepath.unwrap()).map_err(|e| format!("Failed to open file: {}", e))?;
            let mut content = String::new();
            file.read_to_string(&mut content)
                .map_err(|e| format!("Failed to read file: {}", e))?;
            content
        }
    };

    let lines: Vec<String> = content.lines().map(String::from).collect();

    // Detect submission type
    let submission_type = detect_submission_type(&lines[0])?;

    // Get document structure index
    let doc_index = build_document_index(&lines);

    // Parse header metadata
    let header_lines = &lines[..doc_index.header_end];
    let metadata = parse_header_metadata(header_lines, submission_type);

    // Process documents using indexed positions
    let mut documents = Vec::new();
    let mut documents_metadata = Vec::new();

    for &(doc_start, doc_end) in &doc_index.document_positions {
        // Find corresponding text section for this document
        let mut text_start = 0;
        let mut text_end = 0;
        let mut found_text = false;

        for &(start, end) in &doc_index.text_positions {
            if start > doc_start && end < doc_end {
                text_start = start;
                text_end = end;
                found_text = true;
                break;
            }
        }

        if !found_text {
            continue; // Skip documents without text sections
        }

        // Extract document metadata
        let doc_metadata = parse_document_metadata(&lines[doc_start + 1..text_start]);
        documents_metadata.push(doc_metadata);

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

    // Add documents metadata to the main metadata
    let documents_value = Value::Array(
        documents_metadata
            .into_iter()
            .map(|m| Value::Object(m.into_iter().collect()))
            .collect(),
    );

    let mut final_metadata = metadata;
    final_metadata.insert("documents".to_string(), documents_value);

    Ok((final_metadata, documents))
}
