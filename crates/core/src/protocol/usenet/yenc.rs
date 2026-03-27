//! yEnc decoder for Usenet articles.
//!
//! yEnc is the standard encoding for binary files on Usenet.
//! It encodes each byte by adding 42 (mod 256), with escape sequences
//! for special characters (NUL, LF, CR, =, TAB, dot).

use crc32fast::Hasher;

/// Decoded yEnc article data.
#[derive(Debug)]
pub struct YencDecoded {
    pub name: Option<String>,
    pub size: Option<u64>,
    pub part: Option<u32>,
    pub total: Option<u32>,
    pub begin: Option<u64>,
    pub end: Option<u64>,
    pub data: Vec<u8>,
    pub crc32: Option<u32>,
    pub part_crc32: Option<u32>,
}

/// Decode a yEnc-encoded article body.
pub fn decode_yenc(article_body: &[u8]) -> Result<YencDecoded, crate::Error> {
    let mut result = YencDecoded {
        name: None,
        size: None,
        part: None,
        total: None,
        begin: None,
        end: None,
        data: Vec::new(),
        crc32: None,
        part_crc32: None,
    };

    let text = String::from_utf8_lossy(article_body);
    let mut in_body = false;
    let mut lines = text.lines().peekable();

    while let Some(line) = lines.next() {
        if line.starts_with("=ybegin ") {
            // Parse ybegin header
            result.name = extract_yenc_param(line, "name");
            result.size = extract_yenc_param(line, "size").and_then(|s| s.parse().ok());
            result.part = extract_yenc_param(line, "part").and_then(|s| s.parse().ok());
            result.total = extract_yenc_param(line, "total").and_then(|s| s.parse().ok());

            // Check for =ypart header (multipart)
            if let Some(next_line) = lines.peek() {
                if next_line.starts_with("=ypart ") {
                    let part_line = lines.next().unwrap();
                    result.begin = extract_yenc_param(part_line, "begin").and_then(|s| s.parse().ok());
                    result.end = extract_yenc_param(part_line, "end").and_then(|s| s.parse().ok());
                }
            }
            in_body = true;
            continue;
        }

        if line.starts_with("=yend ") {
            // Parse yend trailer
            result.crc32 = extract_yenc_param(line, "crc32")
                .and_then(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).ok());
            result.part_crc32 = extract_yenc_param(line, "pcrc32")
                .and_then(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).ok());
            break;
        }

        if in_body {
            // Decode yEnc data line
            decode_yenc_line(line.as_bytes(), &mut result.data);
        }
    }

    if result.data.is_empty() && !in_body {
        return Err(crate::Error::Other("No yEnc data found in article".into()));
    }

    Ok(result)
}

/// Decode a single yEnc-encoded line.
fn decode_yenc_line(line: &[u8], output: &mut Vec<u8>) {
    let mut i = 0;
    while i < line.len() {
        if line[i] == b'=' && i + 1 < line.len() {
            // Escape sequence: next byte - 64 - 42
            i += 1;
            output.push(line[i].wrapping_sub(64).wrapping_sub(42));
        } else {
            // Normal byte: subtract 42
            output.push(line[i].wrapping_sub(42));
        }
        i += 1;
    }
}

/// Verify CRC32 checksum of decoded data.
pub fn verify_crc32(data: &[u8], expected: u32) -> bool {
    let mut hasher = Hasher::new();
    hasher.update(data);
    hasher.finalize() == expected
}

/// Extract a parameter value from a yEnc header line.
/// Format: `=ybegin line=128 size=123456 name=filename.ext`
fn extract_yenc_param(line: &str, param: &str) -> Option<String> {
    let search = format!("{param}=");
    let start = line.find(&search)? + search.len();

    // "name=" is special: it goes to end of line
    if param == "name" {
        return Some(line[start..].trim().to_string());
    }

    // Other params end at the next space
    let end = line[start..].find(' ').map(|p| start + p).unwrap_or(line.len());
    Some(line[start..end].trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_simple_yenc() {
        // "Hello" encoded with yEnc: each byte + 42
        // H(72)->114='r', e(101)->143, l(108)->150, l(108)->150, o(111)->153
        // Use only ASCII-safe test: "rust" -> r(114)+42=156, u(117)+42=159, s(115)+42=157, t(116)+42=158
        // Those are non-ASCII, so test with bytes that stay ASCII after +42
        // Use bytes that after +42 stay in printable ASCII range: e.g. original byte 1..85
        // Let's encode "AB" = [65, 66] -> [107='k', 108='l']
        let article = b"=ybegin line=128 size=2 name=test.txt\nkl\n=yend size=2";

        let decoded = decode_yenc(article).unwrap();
        assert_eq!(decoded.name.as_deref(), Some("test.txt"));
        assert_eq!(decoded.size, Some(2));
        assert_eq!(decoded.data, b"AB");
    }

    #[test]
    fn test_decode_with_escape() {
        // Test escape sequence: =J decodes to NUL (0)
        // NUL(0) + 42 = 42, which is '=' (critical char), so it's escaped as =J
        // '=' is byte 61, J is byte 74. 74 - 64 - 42 = -32 = 224 (mod 256)...
        // Actually: escaped byte = (original + 42 + 64) mod 256
        // For NUL(0): 0+42 = 42 -> escaped as = followed by 42+64=106='j'
        let article = "=ybegin line=128 size=1 name=test.bin\n=j\n=yend size=1";
        let decoded = decode_yenc(article.as_bytes()).unwrap();
        assert_eq!(decoded.data, vec![0u8]); // NUL byte
    }

    #[test]
    fn test_crc32_verify() {
        let data = b"Hello, World!";
        let mut hasher = Hasher::new();
        hasher.update(data);
        let crc = hasher.finalize();
        assert!(verify_crc32(data, crc));
        assert!(!verify_crc32(data, crc + 1));
    }

    #[test]
    fn test_multipart_yenc() {
        let article = "=ybegin part=1 total=3 line=128 size=30000 name=bigfile.bin\n=ypart begin=1 end=10000\nrrrr\n=yend size=10000 pcrc32=12345678";
        let decoded = decode_yenc(article.as_bytes()).unwrap();
        assert_eq!(decoded.part, Some(1));
        assert_eq!(decoded.total, Some(3));
        assert_eq!(decoded.begin, Some(1));
        assert_eq!(decoded.end, Some(10000));
        assert_eq!(decoded.name.as_deref(), Some("bigfile.bin"));
    }

    #[test]
    fn test_extract_param() {
        assert_eq!(
            extract_yenc_param("=ybegin line=128 size=500 name=file.txt", "size"),
            Some("500".to_string())
        );
        assert_eq!(
            extract_yenc_param("=ybegin line=128 size=500 name=my file.txt", "name"),
            Some("my file.txt".to_string())
        );
    }

    #[test]
    fn test_no_yenc_data() {
        let result = decode_yenc(b"Just some random text\nwithout yenc");
        assert!(result.is_err());
    }
}
