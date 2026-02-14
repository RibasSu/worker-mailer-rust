//! Encoding, decoding, email validation (mirror of TS utils).

use regex::Regex;
use std::str;

/// Validates email format (RFC 5322 simplified).
pub fn is_valid_email(email: &str) -> bool {
    if email.is_empty() {
        return false;
    }
    let re = Regex::new(
        r"^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$",
    )
    .unwrap();
    if !re.is_match(email) {
        return false;
    }
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return false;
    }
    let local = parts[0];
    let domain = parts[1];
    if local.len() > 64 {
        return false;
    }
    if domain.len() > 255 {
        return false;
    }
    if !domain.contains('.') {
        return false;
    }
    let tld = domain.split('.').last().unwrap_or("");
    tld.len() >= 2
}

/// Returns invalid emails from a list.
pub fn validate_emails(emails: &[String]) -> Vec<String> {
    emails
        .iter()
        .filter(|e| !is_valid_email(e))
        .cloned()
        .collect()
}

/// Encode string to UTF-8 bytes.
pub fn encode(s: &str) -> Vec<u8> {
    s.as_bytes().to_vec()
}

/// Decode UTF-8 bytes to string.
pub fn decode(bytes: &[u8]) -> Result<String, std::string::FromUtf8Error> {
    String::from_utf8(bytes.to_vec())
}

/// Quoted-printable encoding (RFC 2045).
pub fn encode_quoted_printable(text: &str, line_length: usize) -> String {
    let bytes = encode(text);
    let mut result = String::new();
    let mut current_line_length = 0;
    let mut i = 0;

    while i < bytes.len() {
        let byte = bytes[i];
        let encoded: String = if byte == 0x0a {
            result.push_str("\r\n");
            current_line_length = 0;
            i += 1;
            continue;
        } else if byte == 0x0d {
            if i + 1 < bytes.len() && bytes[i + 1] == 0x0a {
                result.push_str("\r\n");
                current_line_length = 0;
                i += 2;
                continue;
            } else {
                "=0D".to_string()
            }
        } else {
            let is_whitespace = byte == 0x20 || byte == 0x09;
            let next_is_line_break = i + 1 >= bytes.len()
                || bytes[i + 1] == 0x0a
                || bytes[i + 1] == 0x0d;
            let needs_encoding = (byte < 32 && (byte != 0x20 && byte != 0x09))
                || byte > 126
                || byte == 61
                || (is_whitespace && next_is_line_break);

            if needs_encoding {
                format!("={:02X}", byte)
            } else {
                char::from(byte).to_string()
            }
        };

        if current_line_length + encoded.len() > line_length.saturating_sub(3) {
            result.push_str("=\r\n");
            current_line_length = 0;
        }
        result.push_str(&encoded);
        current_line_length += encoded.len();
        i += 1;
    }

    result
}

/// RFC 2047 header encoding (UTF-8 Q).
pub fn encode_header(text: &str) -> String {
    if !text.chars().any(|c| c as u32 > 127) {
        return text.to_string();
    }
    let bytes = encode(text);
    let mut encoded = String::new();
    for byte in bytes {
        if (33..=126).contains(&byte) && byte != 63 && byte != 61 && byte != 95 {
            encoded.push(char::from(byte));
        } else if byte == 32 {
            encoded.push('_');
        } else {
            encoded.push_str(&format!("={:02X}", byte));
        }
    }
    format!("=?UTF-8?Q?{}?=", encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_email() {
        assert!(is_valid_email("a@b.co"));
        assert!(!is_valid_email(""));
        assert!(!is_valid_email("invalid"));
    }
}
