use std::collections::HashSet;

/// Detect if a line starts with "begin" (UU encoded content)
pub fn detect_uu(first_line: &str) -> bool {
    first_line.trim().starts_with("begin")
}

/// Clean lines by removing leading/trailing whitespace and special tags
pub fn clean_lines(lines: &[String]) -> Vec<String> {
    let special_tags: HashSet<&str> = ["<PDF>", "<XBRL>", "<XML>"].iter().cloned().collect();

    // Skip leading empty lines
    let start = lines
        .iter()
        .position(|line| !line.trim().is_empty())
        .unwrap_or(lines.len());
    if start >= lines.len() {
        return Vec::new();
    }

    let trimmed_lines = &lines[start..];
    let first_line = trimmed_lines[0].trim();

    if special_tags.contains(first_line) {
        let tag = &first_line[1..first_line.len() - 1];
        let end_tag = format!("</{}>", tag);

        // Find closing tag position
        if let Some(end_pos) = trimmed_lines
            .iter()
            .rev()
            .position(|line| line.trim() == end_tag)
            .map(|pos| trimmed_lines.len() - pos - 1)
        {
            return trimmed_lines[1..end_pos].to_vec();
        }
    }

    trimmed_lines.to_vec()
}

/// Process text content, handling UU encoding if necessary
pub fn process_text_content(lines: &[String]) -> Vec<u8> {
    let cleaned_lines = clean_lines(lines);

    if cleaned_lines.is_empty() {
        return Vec::new();
    }

    if detect_uu(&cleaned_lines[0]) {
        // Use uuencode crate for UU decoding
        let content = cleaned_lines.join("\n");
        if let Some((decoded, _)) = uuencode::uudecode(&content) {
            return decoded;
        }
        return Vec::new();
    } else {
        // For regular text content
        cleaned_lines.join("\n").into_bytes()
    }
}

/// Generate a safe filename from a string
pub fn safe_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Generate a default filename for a document
pub fn default_filename(index: usize, is_binary: bool) -> String {
    if is_binary {
        format!("doc_{}.bin", index + 1)
    } else {
        format!("doc_{}.txt", index + 1)
    }
}
