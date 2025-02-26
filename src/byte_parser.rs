use crate::types::{
    fast_map_with_capacity, DocumentIndex, FastMap, MetadataDict, MetadataValue, ParseError, Span,
    SubmissionType, TagScanner, TagType,
};
use crate::uu_decoder;
use memchr::{memchr, memmem};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use std::collections::hash_map::Entry;

// Constants for common tag pattern detection
const DOCUMENT_OPEN: &[u8] = b"<DOCUMENT>";
const DOCUMENT_CLOSE: &[u8] = b"</DOCUMENT>";
const TEXT_OPEN: &[u8] = b"<TEXT>";
const TEXT_CLOSE: &[u8] = b"</TEXT>";
const PRIVACY_MSG: &[u8] = b"-----BEGIN PRIVACY-ENHANCED MESSAGE-----";
const SEC_DOCUMENT: &[u8] = b"<SEC-DOCUMENT>";
const SUBMISSION: &[u8] = b"<SUBMISSION>";
const ANGLE_OPEN: u8 = b'<';
const ANGLE_CLOSE: u8 = b'>';
const NEWLINE: u8 = b'\n';
const CR: u8 = b'\r';
const SPACE: u8 = b' ';
const SLASH: u8 = b'/';
const COLON: u8 = b':';

/// Detect submission type from first line
#[inline]
fn detect_submission_type(data: &[u8]) -> Result<SubmissionType, ParseError> {
    // Find the first line end
    let line_end = memchr(NEWLINE, data).unwrap_or(data.len());
    let first_line = &data[..line_end];

    // Check for known submission types using fast byte comparison
    if memmem::find(first_line, SUBMISSION).is_some() {
        Ok(SubmissionType::DashedDefault)
    } else if memmem::find(first_line, PRIVACY_MSG).is_some() {
        Ok(SubmissionType::TabPrivacy)
    } else if memmem::find(first_line, SEC_DOCUMENT).is_some() {
        Ok(SubmissionType::TabDefault)
    } else {
        // Only allocate a string for error case
        let error_str =
            String::from_utf8_lossy(&first_line[..std::cmp::min(100, first_line.len())])
                .to_string();
        Err(ParseError::UnknownSubmissionType(error_str))
    }
}

/// Fast line indexing
#[inline]
fn index_lines(data: &[u8]) -> Vec<(usize, usize)> {
    let mut line_indices = Vec::with_capacity(data.len() / 50); // Estimate lines
    let mut pos = 0;
    let mut line_start = 0;

    while pos < data.len() {
        if data[pos] == NEWLINE {
            // Handle different line endings
            let line_end = if pos > 0 && data[pos - 1] == CR {
                pos - 1
            } else {
                pos
            };

            line_indices.push((line_start, line_end));
            line_start = pos + 1;
        }
        pos += 1;
    }

    // Add the last line if there's content
    if line_start < data.len() {
        line_indices.push((line_start, data.len()));
    }

    line_indices
}

/// Fast tag detection using memchr and byte comparison
#[inline]
fn find_tag(data: &[u8], tag: &[u8], start_pos: usize) -> Option<usize> {
    memmem::find(&data[start_pos..], tag).map(|pos| pos + start_pos)
}

/// Optimized document structure indexing using byte operations
fn build_document_index(data: &[u8], line_indices: &[(usize, usize)]) -> DocumentIndex {
    let mut index = DocumentIndex::new();
    let mut doc_start: isize = -1;
    let mut text_start: isize = -1;

    // Create SIMD-friendly searchers for common tags
    let doc_open_finder = memmem::Finder::new(DOCUMENT_OPEN);
    let doc_close_finder = memmem::Finder::new(DOCUMENT_CLOSE);
    let text_open_finder = memmem::Finder::new(TEXT_OPEN);
    let text_close_finder = memmem::Finder::new(TEXT_CLOSE);

    // Pre-calculate all document open/close positions
    let all_doc_opens: Vec<_> = doc_open_finder.find_iter(data).collect();
    let all_doc_closes: Vec<_> = doc_close_finder.find_iter(data).collect();
    let all_text_opens: Vec<_> = text_open_finder.find_iter(data).collect();

    // Find header end (first document tag)
    if let Some(&first_doc) = all_doc_opens.first() {
        index.header_end = first_doc;
    }

    // Build document positions with optimized algorithm
    let mut doc_positions = Vec::with_capacity(all_doc_opens.len());
    for (&open_pos, &close_pos) in all_doc_opens.iter().zip(all_doc_closes.iter()) {
        if open_pos < close_pos {
            doc_positions.push((open_pos, close_pos));
        }
    }
    index.document_positions = doc_positions;

    // Find text sections efficiently
    let mut text_positions = Vec::with_capacity(all_text_opens.len());
    for &text_start in &all_text_opens {
        // Find the closest text end
        if let Some(text_end_pos) = text_close_finder
            .find(&data[text_start..])
            .map(|pos| pos + text_start)
        {
            // Check if the next non-whitespace tag is document close
            let mut pos = text_end_pos + TEXT_CLOSE.len();
            while pos < data.len()
                && (data[pos] == SPACE || data[pos] == NEWLINE || data[pos] == CR)
            {
                pos += 1;
            }

            // If we're at a document close tag, record this text position
            if pos + DOCUMENT_CLOSE.len() <= data.len()
                && &data[pos..pos + DOCUMENT_CLOSE.len()] == DOCUMENT_CLOSE
            {
                // Check for text leftovers
                if pos > text_end_pos + TEXT_CLOSE.len() {
                    let leftover_span = Span::new(text_end_pos + TEXT_CLOSE.len(), pos);
                    if !leftover_span.is_empty() {
                        // Only store non-empty leftovers
                        let slice = leftover_span.slice(data);
                        if slice.iter().any(|&b| b != SPACE && b != NEWLINE && b != CR) {
                            index.text_leftovers.insert(text_end_pos, leftover_span);
                        }
                    }
                }

                text_positions.push((text_start, text_end_pos));
            }
        }
    }
    index.text_positions = text_positions;

    index
}

/// Parse tag and content from byte slice
#[inline]
fn parse_tag_content<'a>(data: &'a [u8]) -> Option<(&'a [u8], &'a [u8])> {
    // Find tag end
    let tag_end = memchr(ANGLE_CLOSE, data)?;
    if tag_end < 1 {
        return None;
    }

    let tag = &data[1..tag_end]; // Skip the opening '<'
    let content_start = tag_end + 1;

    // Get content (rest of the slice)
    let content = if content_start < data.len() {
        &data[content_start..]
    } else {
        b""
    };

    Some((tag, content))
}

/// Convert ASCII bytes to lowercase in place using SIMD where available
#[inline]
fn ascii_to_lowercase(bytes: &[u8]) -> SmallVec<[u8; 64]> {
    // For small tags, avoid heap allocation
    let mut result = SmallVec::<[u8; 64]>::with_capacity(bytes.len());
    result.extend(
        bytes
            .iter()
            .map(|&b| if b >= b'A' && b <= b'Z' { b + 32 } else { b }),
    );
    result
}

/// Parse document metadata from byte range
fn parse_document_metadata(data: &[u8], start: usize, end: usize) -> MetadataDict {
    let slice = &data[start..end];
    let mut metadata = FastMap::default();
    let mut current_key = None;

    let lines = index_lines(slice);

    for &(line_start, line_end) in &lines {
        let line = &slice[line_start..line_end];

        if !line.is_empty() && line[0] == ANGLE_OPEN && memchr(ANGLE_CLOSE, line).is_some() {
            if let Some((tag, content)) = parse_tag_content(line) {
                // Convert tag to lowercase efficiently
                let lowercase_tag = ascii_to_lowercase(tag);

                // Trim content
                let content = content.trim_ascii();

                // Store current key and content
                let key = String::from_utf8_lossy(&lowercase_tag).into_owned();
                current_key = Some(key.clone());

                // Only allocate string for non-empty content
                if !content.is_empty() {
                    let content_str = String::from_utf8_lossy(content).into_owned();
                    metadata.insert(key, MetadataValue::Text(content_str));
                }
            }
        } else if let Some(ref key) = current_key {
            // Continuation of previous content
            if let Some(MetadataValue::Text(ref mut text)) = metadata.get_mut(key) {
                // Only process non-empty lines
                if !line.is_empty() {
                    // Append to existing text with space
                    text.push(' ');
                    text.push_str(&String::from_utf8_lossy(line.trim_ascii()));
                }
            }
        }
    }

    metadata
}

/// Trim ASCII whitespace from byte slice
trait TrimAscii {
    fn trim_ascii(&self) -> &Self;
    fn trim_ascii_start(&self) -> &Self;
    fn trim_ascii_end(&self) -> &Self;
}

impl TrimAscii for [u8] {
    fn trim_ascii(&self) -> &Self {
        self.trim_ascii_start().trim_ascii_end()
    }

    fn trim_ascii_start(&self) -> &Self {
        let mut bytes = self;
        while !bytes.is_empty()
            && (bytes[0] == SPACE || bytes[0] == NEWLINE || bytes[0] == CR || bytes[0] == b'\t')
        {
            bytes = &bytes[1..];
        }
        bytes
    }

    fn trim_ascii_end(&self) -> &Self {
        let mut bytes = self;
        while !bytes.is_empty()
            && (bytes[bytes.len() - 1] == SPACE
                || bytes[bytes.len() - 1] == NEWLINE
                || bytes[bytes.len() - 1] == CR
                || bytes[bytes.len() - 1] == b'\t')
        {
            bytes = &bytes[..bytes.len() - 1];
        }
        bytes
    }
}

/// Process text content, handling UU encoding
fn process_text_content(data: &[u8]) -> Vec<u8> {
    // Skip leading whitespace
    let data = data.trim_ascii_start();

    if data.is_empty() {
        return Vec::new();
    }

    // Check for UU encoding by comparing against "begin" prefix
    if data.len() >= 5 && &data[0..5] == b"begin" {
        // UU decode the content
        uu_decoder::decode(data)
    } else {
        // For regular text, just copy the bytes
        data.to_vec()
    }
}

/// Parse dashed default header format
fn parse_dashed_default_header(data: &[u8], end: usize) -> MetadataDict {
    let mut header_metadata = fast_map_with_capacity(50);
    let mut tag_stack: Vec<SmallVec<[u8; 64]>> = Vec::with_capacity(16);
    let mut dict_stack: Vec<MetadataDict> = Vec::with_capacity(16);
    dict_stack.push(header_metadata);

    let lines = index_lines(&data[..end]);

    for (i, &(line_start, line_end)) in lines.iter().enumerate() {
        let line = &data[line_start..line_end];

        // Skip empty lines or lines without angle brackets
        if line.is_empty() || memchr(ANGLE_CLOSE, line).is_none() {
            continue;
        }

        if let Some((tag, content)) = parse_tag_content(line) {
            // Convert tag to lowercase efficiently
            let lowercase_tag = ascii_to_lowercase(tag);

            // Check if this is a closing tag
            if !lowercase_tag.is_empty() && lowercase_tag[0] == SLASH {
                let tag_name = &lowercase_tag[1..]; // Remove slash

                if tag_stack.last().map_or(false, |t| t.as_slice() == tag_name) {
                    tag_stack.pop();
                    dict_stack.pop();
                }
                continue;
            }

            // Look ahead to check if this tag has a closing tag - limit search range
            let search_end = (i + 100).min(lines.len());
            let mut has_closing_tag = false;

            let closing_tag = SmallVec::<[u8; 64]>::from_iter(
                [SLASH].iter().cloned().chain(lowercase_tag.iter().cloned()),
            );

            for j in i + 1..search_end {
                let (next_start, next_end) = lines[j];
                let next_line = &data[next_start..next_end];

                if let Some((next_tag, _)) = parse_tag_content(next_line) {
                    let next_lowercase = ascii_to_lowercase(next_tag);
                    if next_lowercase.as_slice() == closing_tag.as_slice() {
                        has_closing_tag = true;
                        break;
                    }
                }
            }

            // Get current dictionary
            let current_dict = dict_stack.last_mut().unwrap();

            if has_closing_tag {
                // Create nested dict and handle insertion to parent dict
                let nested_dict = fast_map_with_capacity(10);
                let tag_str = String::from_utf8_lossy(&lowercase_tag).into_owned();

                // Efficient insertion using entry API
                match current_dict.entry(tag_str.clone()) {
                    Entry::Occupied(mut entry) => match entry.get_mut() {
                        MetadataValue::Dict(existing) => {
                            let mut list = Vec::with_capacity(2);
                            list.push(MetadataValue::Dict(existing.clone()));
                            list.push(MetadataValue::Dict(nested_dict.clone()));
                            entry.insert(MetadataValue::List(list));
                        }
                        MetadataValue::List(list) => {
                            list.push(MetadataValue::Dict(nested_dict.clone()));
                        }
                        MetadataValue::Text(existing_text) => {
                            let existing_text_clone = existing_text.clone();
                            let list = vec![
                                MetadataValue::Text(existing_text_clone),
                                MetadataValue::Dict(nested_dict.clone()),
                            ];
                            entry.insert(MetadataValue::List(list));
                        }
                    },
                    Entry::Vacant(entry) => {
                        entry.insert(MetadataValue::Dict(nested_dict.clone()));
                    }
                }

                // Push to stacks
                tag_stack.push(lowercase_tag);
                dict_stack.push(nested_dict);
            } else {
                // Process text content
                let trimmed_content = content.trim_ascii();
                if !trimmed_content.is_empty() {
                    let content_str = String::from_utf8_lossy(trimmed_content).into_owned();
                    let tag_str = String::from_utf8_lossy(&lowercase_tag).into_owned();

                    // Efficient insertion using entry API
                    match current_dict.entry(tag_str) {
                        Entry::Occupied(mut entry) => match entry.get_mut() {
                            MetadataValue::Text(existing) => {
                                let existing_clone = existing.clone();
                                let list = vec![
                                    MetadataValue::Text(existing_clone),
                                    MetadataValue::Text(content_str),
                                ];
                                entry.insert(MetadataValue::List(list));
                            }
                            MetadataValue::List(list) => {
                                list.push(MetadataValue::Text(content_str));
                            }
                            MetadataValue::Dict(_) => {
                                let existing = entry.get().clone();
                                let list = vec![existing, MetadataValue::Text(content_str)];
                                entry.insert(MetadataValue::List(list));
                            }
                        },
                        Entry::Vacant(entry) => {
                            entry.insert(MetadataValue::Text(content_str));
                        }
                    }
                }
            }
        }
    }

    // Return the root dictionary without cloning
    if dict_stack.is_empty() {
        FastMap::default()
    } else {
        dict_stack.remove(0)
    }
}

/// Parse tab-formatted header
fn parse_tab_header(data: &[u8], end: usize, submission_type: &SubmissionType) -> MetadataDict {
    let mut header_metadata = fast_map_with_capacity(50);

    // Handle privacy-enhanced message if needed
    if *submission_type == SubmissionType::TabPrivacy {
        let privacy_finder = memmem::Finder::new(PRIVACY_MSG);

        if let Some(start_pos) = privacy_finder.find(&data[..end]) {
            let mut privacy_msg = Vec::new();
            let mut pos = start_pos + PRIVACY_MSG.len();

            // Skip to next line
            while pos < end && data[pos] != NEWLINE {
                pos += 1;
            }
            pos += 1; // Skip the newline

            // Read until we find a line with < and uppercase character
            let mut line_start = pos;
            while pos < end {
                if data[pos] == NEWLINE {
                    let line = &data[line_start..pos];

                    // Check if line contains < and an uppercase character
                    if memchr(ANGLE_OPEN, line).is_some()
                        && line.iter().any(|&b| b >= b'A' && b <= b'Z')
                    {
                        break;
                    }

                    // Add line to privacy message
                    privacy_msg.push(line.to_vec());
                    line_start = pos + 1;
                }
                pos += 1;
            }

            // Join privacy message lines with newlines
            if !privacy_msg.is_empty() {
                let joined = privacy_msg
                    .iter()
                    .flat_map(|line| line.iter().chain([NEWLINE].iter()).cloned())
                    .collect::<Vec<_>>();

                let msg_str = String::from_utf8_lossy(&joined).into_owned();
                header_metadata.insert(
                    "privacy-enhanced-message".to_string(),
                    MetadataValue::Text(msg_str),
                );
            }
        }
    }

    // Use indentation to track nesting
    let mut indent_stack: Vec<usize> = vec![0];
    let mut dict_stack: Vec<MetadataDict> = Vec::with_capacity(16);
    dict_stack.push(header_metadata);

    let lines = index_lines(&data[..end]);

    for &(line_start, line_end) in &lines {
        let line = &data[line_start..line_end];
        if line.is_empty() {
            continue;
        }

        // Calculate indentation
        let indent = line
            .iter()
            .take_while(|&&b| b == SPACE || b == b'\t')
            .count();

        // Parse tag and text
        let (tag, text) = if memchr(ANGLE_CLOSE, line).is_some() {
            if let Some((tag, content)) = parse_tag_content(line) {
                // Convert tag to lowercase efficiently
                let lowercase_tag = ascii_to_lowercase(tag);

                // Skip closing tags
                if !lowercase_tag.is_empty() && lowercase_tag[0] == SLASH {
                    continue;
                }

                (lowercase_tag, content)
            } else {
                continue;
            }
        } else if memchr(COLON, line).is_some() {
            // Handle key-value pairs with colon
            if let Some(pos) = memchr(COLON, line) {
                let tag = &line[..pos];
                let content = if pos + 1 < line.len() {
                    &line[pos + 1..]
                } else {
                    b""
                };

                (ascii_to_lowercase(tag), content)
            } else {
                continue;
            }
        } else {
            continue;
        };

        // Find appropriate parent based on indentation
        while indent_stack.len() > 1 && indent_stack.last().unwrap() >= &indent {
            indent_stack.pop();
            dict_stack.pop();
        }

        let current_dict = dict_stack.last_mut().unwrap();
        let trimmed_text = text.trim_ascii();
        let tag_str = String::from_utf8_lossy(&tag).into_owned();

        if !trimmed_text.is_empty() {
            // Add text value
            let text_str = String::from_utf8_lossy(trimmed_text).into_owned();

            // Use entry API for efficient insertion
            match current_dict.entry(tag_str) {
                Entry::Occupied(mut entry) => match entry.get_mut() {
                    MetadataValue::Text(existing) => {
                        let existing_clone = existing.clone();
                        let list = vec![
                            MetadataValue::Text(existing_clone),
                            MetadataValue::Text(text_str),
                        ];
                        entry.insert(MetadataValue::List(list));
                    }
                    MetadataValue::List(list) => {
                        list.push(MetadataValue::Text(text_str));
                    }
                    MetadataValue::Dict(_) => {
                        let existing = entry.get().clone();
                        let list = vec![existing, MetadataValue::Text(text_str)];
                        entry.insert(MetadataValue::List(list));
                    }
                },
                Entry::Vacant(entry) => {
                    entry.insert(MetadataValue::Text(text_str));
                }
            }
        } else {
            // Create nested dict
            let nested_dict = fast_map_with_capacity(10);

            // Use entry API for efficient insertion
            match current_dict.entry(tag_str) {
                Entry::Occupied(mut entry) => match entry.get_mut() {
                    MetadataValue::Dict(existing) => {
                        let existing_clone = existing.clone();
                        let list = vec![
                            MetadataValue::Dict(existing_clone),
                            MetadataValue::Dict(nested_dict.clone()),
                        ];
                        entry.insert(MetadataValue::List(list));
                    }
                    MetadataValue::List(list) => {
                        list.push(MetadataValue::Dict(nested_dict.clone()));
                    }
                    MetadataValue::Text(_) => {
                        let existing = entry.get().clone();
                        let list = vec![existing, MetadataValue::Dict(nested_dict.clone())];
                        entry.insert(MetadataValue::List(list));
                    }
                },
                Entry::Vacant(entry) => {
                    entry.insert(MetadataValue::Dict(nested_dict.clone()));
                }
            }

            // Push new context to stacks
            indent_stack.push(indent);
            dict_stack.push(nested_dict);
        }
    }

    // Return the root dictionary without cloning
    if dict_stack.is_empty() {
        FastMap::default()
    } else {
        dict_stack.remove(0)
    }
}

/// Main parsing function - processes a byte array and returns metadata and documents
pub fn parse_sgml_bytes(data: &[u8]) -> Result<(MetadataDict, Vec<Vec<u8>>), ParseError> {
    if data.is_empty() {
        return Err(ParseError::InvalidContent("Empty content".to_string()));
    }

    // Detect submission type
    let submission_type = detect_submission_type(data)?;

    // Create line index
    let lines = index_lines(data);

    // Get document structure index
    let doc_index = build_document_index(data, &lines);

    // Parse header metadata
    let mut metadata = match submission_type {
        SubmissionType::DashedDefault => parse_dashed_default_header(data, doc_index.header_end),
        _ => parse_tab_header(data, doc_index.header_end, &submission_type),
    };

    // Create fast lookup map for text positions
    let mut text_position_map = fast_map_with_capacity(doc_index.text_positions.len());
    for &(start, end) in &doc_index.text_positions {
        text_position_map.insert(start, end);
    }

    // Process documents using indexed positions
    let mut documents = Vec::with_capacity(doc_index.document_positions.len());
    let mut doc_metadata_list = Vec::with_capacity(doc_index.document_positions.len());

    for &(doc_start, doc_end) in &doc_index.document_positions {
        // Find corresponding text section efficiently
        let mut text_range = None;

        // Scan for TEXT tag within document bounds
        if let Some(text_start_pos) = find_tag(data, TEXT_OPEN, doc_start) {
            if text_start_pos < doc_end && text_position_map.contains_key(&text_start_pos) {
                let text_end = text_position_map[&text_start_pos];
                text_range = Some((text_start_pos, text_end));
            }
        }

        if let Some((text_start, text_end)) = text_range {
            // Extract document metadata (start+len of DOCUMENT tag to start of TEXT tag)
            let doc_metadata =
                parse_document_metadata(data, doc_start + DOCUMENT_OPEN.len(), text_start);
            doc_metadata_list.push(MetadataValue::Dict(doc_metadata));

            // Get text content (start+len of TEXT tag to end of TEXT)
            let text_content = &data[text_start + TEXT_OPEN.len()..text_end];

            // Handle leftovers after TEXT tag
            let processed_content =
                if let Some(leftover_span) = doc_index.text_leftovers.get(&text_end) {
                    // Join text content with leftover
                    let mut combined = Vec::with_capacity(text_content.len() + leftover_span.len());
                    combined.extend_from_slice(text_content);
                    combined.extend_from_slice(leftover_span.slice(data));
                    process_text_content(&combined)
                } else {
                    process_text_content(text_content)
                };

            documents.push(processed_content);
        }
    }

    // Add document metadata to the metadata dictionary
    metadata.insert(
        "documents".to_string(),
        MetadataValue::List(doc_metadata_list),
    );

    Ok((metadata, documents))
}
