use memchr::memchr;
use smallvec::SmallVec;

/// UU-decode a byte array
pub fn decode(input: &[u8]) -> Vec<u8> {
    // Find the "begin" line and skip it
    let begin_pos = find_begin_line(input);
    if begin_pos.is_none() {
        return Vec::new();
    }

    // Process the content
    let mut result = Vec::with_capacity(input.len() / 4 * 3); // Pre-allocate with reasonable capacity
    let mut pos = begin_pos.unwrap();
    let mut line_start = pos;

    while pos < input.len() {
        // Find the end of the line
        match memchr(b'\n', &input[pos..]) {
            Some(offset) => {
                let line_end = pos + offset;
                let line = &input[line_start..line_end];

                // Skip empty lines
                if !line.is_empty() {
                    let trimmed = trim_ascii(line);

                    // Check for "end" marker
                    if trimmed == b"end" {
                        break;
                    }

                    // Decode the line
                    if let Some(decoded) = decode_uu_line(trimmed) {
                        result.extend_from_slice(&decoded);
                    }
                }

                pos = line_end + 1;
                line_start = pos;
            }
            None => {
                // Last line without newline
                let line = &input[line_start..];
                let trimmed = trim_ascii(line);

                if trimmed != b"end" && !trimmed.is_empty() {
                    if let Some(decoded) = decode_uu_line(trimmed) {
                        result.extend_from_slice(&decoded);
                    }
                }
                break;
            }
        }
    }

    result
}

/// Find the "begin" line and return the position after it
fn find_begin_line(input: &[u8]) -> Option<usize> {
    let begin = b"begin";
    let begin_finder = memchr::memmem::Finder::new(begin);

    if let Some(pos) = begin_finder.find(input) {
        // Find the end of this line
        if let Some(end_pos) = memchr(b'\n', &input[pos..]) {
            return Some(pos + end_pos + 1);
        }
    }

    None
}

/// Trim ASCII whitespace from a byte slice
fn trim_ascii(input: &[u8]) -> &[u8] {
    let mut start = 0;
    let mut end = input.len();

    // Trim start
    while start < end
        && (input[start] == b' '
            || input[start] == b'\t'
            || input[start] == b'\r'
            || input[start] == b'\n')
    {
        start += 1;
    }

    // Trim end
    while end > start
        && (input[end - 1] == b' '
            || input[end - 1] == b'\t'
            || input[end - 1] == b'\r'
            || input[end - 1] == b'\n')
    {
        end -= 1;
    }

    &input[start..end]
}

/// Decode a single UU-encoded line
fn decode_uu_line(line: &[u8]) -> Option<SmallVec<[u8; 128]>> {
    if line.is_empty() {
        return None;
    }

    // First byte indicates the decoded length
    let nbytes = (line[0] - 32) & 0x3F;
    if nbytes == 0 {
        // Empty line
        return Some(SmallVec::new());
    }

    // Calculate how many encoded bytes we need to process (includes the length byte)
    let needed_bytes = (nbytes as usize * 4 + 2) / 3 + 1;

    // Check if we have enough bytes and handle broken encoders
    let effective_bytes = if needed_bytes <= line.len() {
        needed_bytes
    } else {
        // Handle truncated lines with workaround for broken uuencoders
        std::cmp::min(line.len(), needed_bytes)
    };

    // Prepare the output buffer with capacity
    let mut result = SmallVec::<[u8; 128]>::with_capacity(nbytes as usize);

    // Process 4 bytes at a time (3 decoded bytes)
    let mut i = 1; // Skip the length byte
    while i + 3 < effective_bytes && result.len() < nbytes as usize {
        // Decode a group of 4 bytes into 3 output bytes
        let c1 = (line[i] - 32) & 0x3F;
        let c2 = (line[i + 1] - 32) & 0x3F;
        let c3 = (line[i + 2] - 32) & 0x3F;
        let c4 = (line[i + 3] - 32) & 0x3F;

        // Output byte 1: 6 bits from c1 + 2 high bits from c2
        result.push((c1 << 2) | (c2 >> 4));

        // Output byte 2: 4 low bits from c2 + 4 high bits from c3
        if result.len() < nbytes as usize {
            result.push(((c2 & 0x0F) << 4) | (c3 >> 2));
        }

        // Output byte 3: 2 low bits from c3 + 6 bits from c4
        if result.len() < nbytes as usize {
            result.push(((c3 & 0x03) << 6) | c4);
        }

        i += 4;
    }

    // Handle any remaining bytes for incomplete groups
    if i < effective_bytes && result.len() < nbytes as usize {
        let remaining = effective_bytes - i;

        if remaining >= 1 {
            let c1 = (line[i] - 32) & 0x3F;

            if remaining >= 2 {
                let c2 = (line[i + 1] - 32) & 0x3F;
                // Output byte 1
                result.push((c1 << 2) | (c2 >> 4));

                if remaining >= 3 {
                    let c3 = (line[i + 2] - 32) & 0x3F;
                    // Output byte 2
                    if result.len() < nbytes as usize {
                        result.push(((c2 & 0x0F) << 4) | (c3 >> 2));
                    }
                }
            }
        }
    }

    // Ensure we don't exceed the expected length
    if result.len() > nbytes as usize {
        result.truncate(nbytes as usize);
    }

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uu_decode_basic() {
        let input = b"begin 644 test.txt\nM5&AE('1E<W0N\n`\nend\n";
        let expected = b"The test.";

        let result = decode(input);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_uu_decode_empty() {
        let input = b"begin 644 empty.txt\n`\nend\n";
        let expected = b"";

        let result = decode(input);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_uu_decode_partial_line() {
        // Line with partial group at the end
        let input = b"begin 644 partial.txt\n#0V\n`\nend\n";
        let expected = b"A";

        let result = decode(input);
        assert_eq!(result, expected);
    }
}
