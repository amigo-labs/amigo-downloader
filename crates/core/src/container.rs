//! DLC, CCF, and RSDF container import/export.
//!
//! DLC (Download Link Container) uses AES-128-CBC encryption with Base64 encoding.
//! The format is used by JDownloader, pyLoad, and other download managers.
//!
//! ## DLC Format
//! - The file contains Base64-encoded, AES-128-CBC encrypted XML
//! - Key derivation uses a service endpoint (or known key for offline mode)
//! - The decrypted XML contains `<package>` elements with `<file>` children
//! - Each `<file>` has a Base64-encoded `<url>`, optional `<filename>` and `<size>`

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerLink {
    pub url: String,
    pub filename: Option<String>,
    pub filesize: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerPackage {
    pub name: String,
    pub links: Vec<ContainerLink>,
}

/// Decode Base64 (standard alphabet, may have padding).
fn b64_decode(input: &str) -> Result<Vec<u8>, crate::Error> {
    // Simple Base64 decoder — avoids pulling in an extra crate
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut table = [255u8; 256];
    for (i, &c) in alphabet.iter().enumerate() {
        table[c as usize] = i as u8;
    }

    let mut buf = Vec::new();

    let filtered: Vec<u8> = input
        .bytes()
        .filter(|&c| c != b'=' && c != b'\n' && c != b'\r' && c != b' ')
        .collect();

    for chunk in filtered.chunks(4) {
        let mut vals = [0u8; 4];
        for (i, &c) in chunk.iter().enumerate() {
            let v = table[c as usize];
            if v == 255 {
                return Err(crate::Error::Other(format!(
                    "Invalid Base64 character: 0x{c:02x}"
                )));
            }
            vals[i] = v;
        }

        if chunk.len() >= 2 {
            buf.push((vals[0] << 2) | (vals[1] >> 4));
        }
        if chunk.len() >= 3 {
            buf.push((vals[1] << 4) | (vals[2] >> 2));
        }
        if chunk.len() >= 4 {
            buf.push((vals[2] << 6) | vals[3]);
        }
    }

    Ok(buf)
}

/// Encode bytes to Base64.
fn b64_encode(input: &[u8]) -> String {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();

    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };

        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(alphabet[((triple >> 18) & 0x3F) as usize] as char);
        result.push(alphabet[((triple >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            result.push(alphabet[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(alphabet[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }

    result
}

/// Parse the decrypted DLC XML and extract packages/links.
fn parse_dlc_xml(xml: &str) -> Result<Vec<ContainerPackage>, crate::Error> {
    let mut packages = Vec::new();

    // Simple XML parsing without pulling in a full XML crate
    // DLC XML format: <dlc><content><package name="..."><file><url>BASE64</url><filename>BASE64</filename><size>N</size></file></package></content></dlc>
    let xml = xml.trim();

    // Extract packages
    let mut pos = 0;
    while let Some(pkg_start) = xml[pos..].find("<package") {
        let abs_start = pos + pkg_start;

        // Extract package name
        let name = extract_attr(&xml[abs_start..], "name").unwrap_or_else(|| "Default".to_string());

        let pkg_end = match xml[abs_start..].find("</package>") {
            Some(p) => abs_start + p,
            None => break,
        };

        let pkg_content = &xml[abs_start..pkg_end];

        let mut links = Vec::new();
        let mut fpos = 0;
        while let Some(file_start) = pkg_content[fpos..].find("<file") {
            let abs_fstart = fpos + file_start;
            let file_end = match pkg_content[abs_fstart..].find("</file>") {
                Some(p) => abs_fstart + p,
                None => break,
            };

            let file_content = &pkg_content[abs_fstart..file_end];

            if let Some(url_b64) = extract_tag(file_content, "url") {
                let url = String::from_utf8(b64_decode(&url_b64)?)
                    .map_err(|e| crate::Error::Other(e.to_string()))?;

                let filename = extract_tag(file_content, "filename")
                    .and_then(|f| String::from_utf8(b64_decode(&f).ok()?).ok())
                    .map(|f| crate::sanitize_filename(&f));

                let filesize =
                    extract_tag(file_content, "size").and_then(|s| s.parse::<u64>().ok());

                links.push(ContainerLink {
                    url,
                    filename,
                    filesize,
                });
            }

            fpos = file_end + 7; // skip </file>
        }

        if !links.is_empty() {
            packages.push(ContainerPackage { name, links });
        }

        pos = pkg_end + 10; // skip </package>
    }

    // If no packages found, try flat file list (some DLCs don't use packages)
    if packages.is_empty() {
        let mut links = Vec::new();
        let mut fpos = 0;
        while let Some(file_start) = xml[fpos..].find("<file") {
            let abs_fstart = fpos + file_start;
            let file_end = match xml[abs_fstart..].find("</file>") {
                Some(p) => abs_fstart + p,
                None => break,
            };

            let file_content = &xml[abs_fstart..file_end];

            if let Some(url_b64) = extract_tag(file_content, "url")
                && let Ok(url_bytes) = b64_decode(&url_b64)
                    && let Ok(url) = String::from_utf8(url_bytes) {
                        let filename = extract_tag(file_content, "filename")
                            .and_then(|f| String::from_utf8(b64_decode(&f).ok()?).ok());
                        let filesize =
                            extract_tag(file_content, "size").and_then(|s| s.parse::<u64>().ok());
                        links.push(ContainerLink {
                            url,
                            filename,
                            filesize,
                        });
                    }

            fpos = file_end + 7;
        }

        if !links.is_empty() {
            packages.push(ContainerPackage {
                name: "Default".to_string(),
                links,
            });
        }
    }

    Ok(packages)
}

/// Generate DLC XML from packages.
fn generate_dlc_xml(packages: &[ContainerPackage]) -> String {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<dlc>\n<content>\n");

    for pkg in packages {
        xml.push_str(&format!("<package name=\"{}\">\n", escape_xml(&pkg.name)));
        for link in &pkg.links {
            xml.push_str("<file>\n");
            xml.push_str(&format!("<url>{}</url>\n", b64_encode(link.url.as_bytes())));
            if let Some(ref fname) = link.filename {
                xml.push_str(&format!(
                    "<filename>{}</filename>\n",
                    b64_encode(fname.as_bytes())
                ));
            }
            if let Some(size) = link.filesize {
                xml.push_str(&format!("<size>{size}</size>\n"));
            }
            xml.push_str("</file>\n");
        }
        xml.push_str("</package>\n");
    }

    xml.push_str("</content>\n</dlc>");
    xml
}

/// Import a DLC file: decode Base64, extract links.
/// Note: Full AES decryption requires the DLC service key exchange.
/// This implementation handles the common "plain" DLC format where
/// the content is simply Base64-encoded XML.
pub fn import_dlc(data: &[u8]) -> Result<Vec<ContainerPackage>, crate::Error> {
    let text = std::str::from_utf8(data)
        .map_err(|e| crate::Error::Other(format!("Invalid UTF-8 in DLC: {e}")))?
        .trim();

    // Try decoding as Base64 → XML
    if let Ok(decoded) = b64_decode(text)
        && let Ok(xml) = String::from_utf8(decoded)
        && (xml.contains("<dlc") || xml.contains("<file"))
    {
        return parse_dlc_xml(&xml);
    }

    // Maybe it's already plain XML
    if text.contains("<dlc") || text.contains("<file") {
        return parse_dlc_xml(text);
    }

    Err(crate::Error::Other(
        "Could not parse DLC container: unrecognized format".into(),
    ))
}

/// Export download links as a DLC file (Base64-encoded XML).
pub fn export_dlc(packages: &[ContainerPackage]) -> Result<Vec<u8>, crate::Error> {
    let xml = generate_dlc_xml(packages);
    Ok(b64_encode(xml.as_bytes()).into_bytes())
}

/// Import a CCF container (legacy format, stub).
pub fn import_ccf(_data: &[u8]) -> Result<Vec<ContainerPackage>, crate::Error> {
    Err(crate::Error::Other("CCF import not yet implemented".into()))
}

/// Import an RSDF container (legacy format, stub).
pub fn import_rsdf(_data: &[u8]) -> Result<Vec<ContainerPackage>, crate::Error> {
    Err(crate::Error::Other(
        "RSDF import not yet implemented".into(),
    ))
}

// --- Helpers ---

fn extract_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].trim().to_string())
}

fn extract_attr(xml: &str, attr: &str) -> Option<String> {
    let pattern = format!("{attr}=\"");
    let start = xml.find(&pattern)? + pattern.len();
    let end = xml[start..].find('"')? + start;
    Some(xml[start..end].to_string())
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_b64_roundtrip() {
        let original = "Hello, World!";
        let encoded = b64_encode(original.as_bytes());
        let decoded = b64_decode(&encoded).unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), original);
    }

    #[test]
    fn test_export_import_roundtrip() {
        let packages = vec![ContainerPackage {
            name: "Test Package".to_string(),
            links: vec![
                ContainerLink {
                    url: "https://example.com/file1.zip".to_string(),
                    filename: Some("file1.zip".to_string()),
                    filesize: Some(1024),
                },
                ContainerLink {
                    url: "https://example.com/file2.zip".to_string(),
                    filename: Some("file2.zip".to_string()),
                    filesize: None,
                },
            ],
        }];

        let exported = export_dlc(&packages).unwrap();
        let imported = import_dlc(&exported).unwrap();

        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].name, "Test Package");
        assert_eq!(imported[0].links.len(), 2);
        assert_eq!(imported[0].links[0].url, "https://example.com/file1.zip");
        assert_eq!(imported[0].links[0].filename.as_deref(), Some("file1.zip"));
        assert_eq!(imported[0].links[0].filesize, Some(1024));
        assert_eq!(imported[0].links[1].url, "https://example.com/file2.zip");
    }

    #[test]
    fn test_parse_plain_xml() {
        let xml = r#"<dlc><content><package name="My Files"><file><url>aHR0cHM6Ly9leGFtcGxlLmNvbS90ZXN0LnppcA==</url><filename>dGVzdC56aXA=</filename><size>2048</size></file></package></content></dlc>"#;
        let result = import_dlc(xml.as_bytes()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].links[0].url, "https://example.com/test.zip");
        assert_eq!(result[0].links[0].filename.as_deref(), Some("test.zip"));
        assert_eq!(result[0].links[0].filesize, Some(2048));
    }
}
