// uu_decode.rs - Using the uuencode crate correctly
use uuencode::uudecode;

/// Decode UU-encoded content from a list of strings
pub fn decode(lines: &[String]) -> Vec<u8> {
    // Join lines into a single string since the uudecode function expects the entire content
    let content = lines.join("\n");

    // Use the uudecode function from the uuencode crate
    // It returns Option<(Vec<u8>, String)> where the first element is the decoded data
    match uudecode(&content) {
        Some((decoded, _filename)) => decoded,
        None => {
            // Fallback to a simple implementation for robustness
            fallback_decode(lines)
        }
    }
}

// Fallback implementation in case the crate doesn't handle something correctly
fn fallback_decode(lines: &[String]) -> Vec<u8> {
    let mut result = Vec::new();
    let mut in_data = false;

    for line in lines {
        let trimmed = line.trim();

        // Look for begin marker
        if !in_data {
            if trimmed.starts_with("begin") {
                in_data = true;
            }
            continue;
        }

        // Stop at empty line or "end" marker
        if trimmed.is_empty() || trimmed == "end" {
            break;
        }

        // Skip lines that are too short
        if trimmed.len() < 2 {
            continue;
        }

        // Get line length
        let bytes = trimmed.as_bytes();
        let length = (bytes[0] - b' ') & 0x3F;
        if length == 0 {
            continue;
        }

        // Decode this line
        let mut line_data = Vec::with_capacity(length as usize);
        let mut leftbits = 0;
        let mut leftchar: u32 = 0;

        for &ch in &bytes[1..] {
            if ch == b'\n' || ch == b'\r' {
                continue;
            }

            if ch < b' ' || ch > b' ' + 64 {
                continue; // Skip invalid characters
            }

            let val = (ch - b' ') & 0x3F;

            // Shift in 6 bits
            leftchar = (leftchar << 6) | (val as u32);
            leftbits += 6;

            // Extract whole bytes
            if leftbits >= 8 {
                leftbits -= 8;
                line_data.push(((leftchar >> leftbits) & 0xFF) as u8);
                leftchar &= (1 << leftbits) - 1;

                if line_data.len() >= length as usize {
                    break; // Got enough bytes for this line
                }
            }
        }

        result.extend_from_slice(&line_data);
    }

    result
}
