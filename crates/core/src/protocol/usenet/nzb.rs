//! NZB file parser.
//!
//! NZB is an XML format that describes Usenet articles to download.
//! Each NZB contains files, each file contains segments (articles).

use serde::{Deserialize, Serialize};

/// A parsed NZB file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nzb {
    pub files: Vec<NzbFile>,
}

/// A file described in the NZB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NzbFile {
    pub subject: String,
    pub poster: String,
    pub date: Option<u64>,
    pub groups: Vec<String>,
    pub segments: Vec<NzbSegment>,
}

/// A single segment (article) of a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NzbSegment {
    pub number: u32,
    pub bytes: u64,
    pub message_id: String,
}

impl NzbFile {
    /// Extract filename from subject line.
    /// Common format: "filename.ext" yEnc (1/10)
    pub fn filename(&self) -> String {
        // Try to extract quoted filename
        if let Some(start) = self.subject.find('"')
            && let Some(end) = self.subject[start + 1..].find('"') {
                return crate::sanitize_filename(&self.subject[start + 1..start + 1 + end]);
            }
        // Fallback: use subject as-is, sanitized
        crate::sanitize_filename(&self.subject)
    }

    /// Total size in bytes (sum of all segments).
    pub fn total_bytes(&self) -> u64 {
        self.segments.iter().map(|s| s.bytes).sum()
    }
}

/// Parse an NZB XML file.
pub fn parse_nzb(data: &str) -> Result<Nzb, crate::Error> {
    let mut files = Vec::new();
    let data = data.trim();

    // Simple XML parser for NZB format
    let mut pos = 0;
    while let Some(file_start) = data[pos..].find("<file") {
        let abs_start = pos + file_start;

        // Extract attributes
        let tag_end = data[abs_start..]
            .find('>')
            .map(|p| abs_start + p)
            .unwrap_or(data.len());
        let tag = &data[abs_start..tag_end];

        let subject = extract_xml_attr(tag, "subject").unwrap_or_default();
        let poster = extract_xml_attr(tag, "poster").unwrap_or_default();
        let date = extract_xml_attr(tag, "date").and_then(|d| d.parse::<u64>().ok());

        let file_end = match data[abs_start..].find("</file>") {
            Some(p) => abs_start + p,
            None => {
                // Malformed NZB — skip to end
                break;
            }
        };
        let file_content = &data[abs_start..file_end];

        // Parse groups
        let groups = parse_groups(file_content);

        // Parse segments
        let segments = parse_segments(file_content);

        files.push(NzbFile {
            subject,
            poster,
            date,
            groups,
            segments,
        });

        pos = file_end + 7;
    }

    if files.is_empty() {
        return Err(crate::Error::Other("No files found in NZB".into()));
    }

    Ok(Nzb { files })
}

fn parse_groups(xml: &str) -> Vec<String> {
    let mut groups = Vec::new();
    let mut pos = 0;
    while let Some(start) = xml[pos..].find("<group>") {
        let abs_start = pos + start + 7;
        if let Some(end) = xml[abs_start..].find("</group>") {
            let group = xml[abs_start..abs_start + end].trim().to_string();
            if !group.is_empty() {
                groups.push(group);
            }
            pos = abs_start + end + 8;
        } else {
            break;
        }
    }
    groups
}

fn parse_segments(xml: &str) -> Vec<NzbSegment> {
    let mut segments = Vec::new();
    let mut pos = 0;
    while let Some(start) = xml[pos..].find("<segment ") {
        let abs_start = pos + start;
        let tag_end = xml[abs_start..]
            .find('>')
            .map(|p| abs_start + p)
            .unwrap_or(xml.len());
        let tag = &xml[abs_start..tag_end];

        let number = extract_xml_attr(tag, "number")
            .and_then(|n| n.parse::<u32>().ok())
            .unwrap_or(0);
        let bytes = extract_xml_attr(tag, "bytes")
            .and_then(|b| b.parse::<u64>().ok())
            .unwrap_or(0);

        // Message-ID is the text content
        let content_start = tag_end + 1;
        if let Some(seg_end) = xml[content_start..].find("</segment>") {
            let message_id = xml[content_start..content_start + seg_end]
                .trim()
                .to_string();
            segments.push(NzbSegment {
                number,
                bytes,
                message_id,
            });
            pos = content_start + seg_end + 10;
        } else {
            break;
        }
    }

    segments.sort_by_key(|s| s.number);
    segments
}

fn extract_xml_attr(tag: &str, attr: &str) -> Option<String> {
    let pattern = format!("{attr}=\"");
    let start = tag.find(&pattern)? + pattern.len();
    let end = tag[start..].find('"')? + start;
    Some(unescape_xml(&tag[start..end]))
}

fn unescape_xml(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_NZB: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE nzb PUBLIC "-//newzBin//DTD NZB 1.1//EN" "http://www.newzbin.com/DTD/nzb/nzb-1.1.dtd">
<nzb xmlns="http://www.newzbin.com/DTD/2003/nzb">
  <file poster="user@example.com" date="1234567890" subject="&quot;test_file.rar&quot; yEnc (1/3)">
    <groups>
      <group>alt.binaries.test</group>
    </groups>
    <segments>
      <segment bytes="500000" number="1">article1@example.com</segment>
      <segment bytes="500000" number="2">article2@example.com</segment>
      <segment bytes="100000" number="3">article3@example.com</segment>
    </segments>
  </file>
  <file poster="user@example.com" date="1234567890" subject="&quot;test_file.rar&quot; yEnc (2/3)">
    <groups>
      <group>alt.binaries.test</group>
    </groups>
    <segments>
      <segment bytes="600000" number="1">article4@example.com</segment>
    </segments>
  </file>
</nzb>"#;

    #[test]
    fn test_parse_nzb() {
        let nzb = parse_nzb(TEST_NZB).unwrap();
        assert_eq!(nzb.files.len(), 2);

        let f1 = &nzb.files[0];
        assert_eq!(f1.filename(), "test_file.rar");
        assert_eq!(f1.poster, "user@example.com");
        assert_eq!(f1.groups, vec!["alt.binaries.test"]);
        assert_eq!(f1.segments.len(), 3);
        assert_eq!(f1.segments[0].message_id, "article1@example.com");
        assert_eq!(f1.segments[0].number, 1);
        assert_eq!(f1.segments[0].bytes, 500000);
        assert_eq!(f1.total_bytes(), 1100000);

        let f2 = &nzb.files[1];
        assert_eq!(f2.segments.len(), 1);
        assert_eq!(f2.total_bytes(), 600000);
    }

    #[test]
    fn test_empty_nzb() {
        let result = parse_nzb("<nzb></nzb>");
        assert!(result.is_err());
    }
}
