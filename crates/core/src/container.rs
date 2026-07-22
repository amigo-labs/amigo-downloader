//! DLC container import/export.
//!
//! ## What is supported
//! This module handles the **plain** (unencrypted) DLC form: a Base64-encoded
//! XML document listing `<package>`/`<file>` entries. That is what
//! [`export_dlc`] produces and what [`import_dlc`] reads back.
//!
//! ## What is NOT supported (and why)
//! Encrypted DLC files cannot be decrypted here. The classic DLC format derives
//! its AES key via a key-exchange with JDownloader's DLC web service, which has
//! been offline for years — there is no offline key for the general case, so
//! `import_dlc` rejects an encrypted payload rather than pretend to handle it.
//! CCF and RSDF (see [`import_ccf`] / [`import_rsdf`]) are likewise not
//! implemented in the core here.
//!
//! ## Plain DLC format
//! - Base64-encoded XML (no encryption layer)
//! - `<package>` elements with `<file>` children
//! - Each `<file>` has a Base64-encoded `<url>`, optional `<filename>`/`<size>`

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use tracing::warn;

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

/// Decode standard-alphabet Base64 (with padding), tolerating embedded
/// whitespace (DLC producers sometimes wrap the payload across lines).
fn b64_decode(input: &str) -> Result<Vec<u8>, crate::Error> {
    let filtered: String = input.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    STANDARD
        .decode(filtered.as_bytes())
        .map_err(|e| crate::Error::Other(format!("Invalid Base64: {e}")))
}

/// Encode bytes to standard-alphabet Base64 (with padding).
fn b64_encode(input: &[u8]) -> String {
    STANDARD.encode(input)
}

/// Which text-bearing `<file>` child we are currently reading.
enum FileField {
    Url,
    Filename,
    Size,
}

/// Parse the decrypted DLC XML and extract packages/links.
///
/// DLC XML format:
/// `<dlc><content><package name="..."><file><url>BASE64</url>
/// <filename>BASE64</filename><size>N</size></file></package></content></dlc>`
///
/// Uses a real streaming XML reader so element names match exactly — a
/// `<packages>` wrapper is no longer mistaken for a `<package>` element (#39),
/// and a `<filename>` decode failure is surfaced as a warning instead of being
/// silently dropped (#35). Files that appear outside any `<package>` are
/// collected into a synthesized "Default" package.
fn parse_dlc_xml(xml: &str) -> Result<Vec<ContainerPackage>, crate::Error> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_str(xml);

    let mut packages: Vec<ContainerPackage> = Vec::new();
    let mut flat_links: Vec<ContainerLink> = Vec::new();
    let mut current_pkg: Option<ContainerPackage> = None;

    let mut in_file = false;
    let mut file_url: Option<String> = None;
    let mut file_name: Option<String> = None;
    let mut file_size: Option<u64> = None;
    let mut field: Option<FileField> = None;
    let mut text_buf = String::new();

    loop {
        match reader
            .read_event()
            .map_err(|e| crate::Error::Other(format!("Malformed DLC XML: {e}")))?
        {
            Event::Eof => break,
            Event::Start(e) => match e.local_name().as_ref() {
                b"package" => {
                    let name = e
                        .try_get_attribute("name")
                        .ok()
                        .flatten()
                        .and_then(|a| a.unescape_value().ok().map(|v| v.into_owned()))
                        .unwrap_or_else(|| "Default".to_string());
                    current_pkg = Some(ContainerPackage {
                        name,
                        links: Vec::new(),
                    });
                }
                b"file" => {
                    in_file = true;
                    file_url = None;
                    file_name = None;
                    file_size = None;
                }
                b"url" if in_file => {
                    field = Some(FileField::Url);
                    text_buf.clear();
                }
                b"filename" if in_file => {
                    field = Some(FileField::Filename);
                    text_buf.clear();
                }
                b"size" if in_file => {
                    field = Some(FileField::Size);
                    text_buf.clear();
                }
                _ => {}
            },
            Event::Text(e) if field.is_some() => {
                let t = e
                    .unescape()
                    .map_err(|err| crate::Error::Other(format!("Malformed DLC XML text: {err}")))?;
                text_buf.push_str(&t);
            }
            Event::End(e) => match e.local_name().as_ref() {
                b"url" => {
                    if matches!(field, Some(FileField::Url)) {
                        file_url = Some(text_buf.trim().to_string());
                        field = None;
                    }
                }
                b"filename" => {
                    if matches!(field, Some(FileField::Filename)) {
                        file_name = Some(text_buf.trim().to_string());
                        field = None;
                    }
                }
                b"size" => {
                    if matches!(field, Some(FileField::Size)) {
                        file_size = text_buf.trim().parse::<u64>().ok();
                        field = None;
                    }
                }
                b"file" => {
                    in_file = false;
                    if let Some(url_b64) = file_url.take() {
                        // The URL is required; a decode error here is fatal.
                        let url = String::from_utf8(b64_decode(&url_b64)?)
                            .map_err(|e| crate::Error::Other(e.to_string()))?;
                        // A filename decode failure is non-fatal — warn and drop
                        // the name rather than failing the whole import.
                        let filename = file_name.take().and_then(decode_filename);
                        let link = ContainerLink {
                            url,
                            filename,
                            filesize: file_size.take(),
                        };
                        match current_pkg.as_mut() {
                            Some(pkg) => pkg.links.push(link),
                            None => flat_links.push(link),
                        }
                    }
                }
                b"package" => {
                    if let Some(pkg) = current_pkg.take()
                        && !pkg.links.is_empty()
                    {
                        packages.push(pkg);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    if !flat_links.is_empty() {
        packages.push(ContainerPackage {
            name: "Default".to_string(),
            links: flat_links,
        });
    }

    Ok(packages)
}

/// Decode a Base64 `<filename>` value, sanitizing the result. Returns `None`
/// (with a warning) on invalid Base64 or non-UTF-8 rather than dropping it
/// silently, so a bad filename doesn't quietly leave a file unnamed.
fn decode_filename(b64: String) -> Option<String> {
    match b64_decode(&b64) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(s) => Some(crate::sanitize_filename(&s)),
            Err(e) => {
                warn!("Ignoring DLC filename with invalid UTF-8: {e}");
                None
            }
        },
        Err(e) => {
            warn!("Ignoring DLC filename with invalid Base64: {e}");
            None
        }
    }
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

    // Regression for #39: a `<packages>` wrapper element (as produced by some
    // JDownloader exports) must not be mistaken for a `<package>` element. The
    // inner package's name and links must parse correctly.
    #[test]
    fn test_packages_wrapper_is_not_mistaken_for_a_package() {
        let url = b64_encode(b"https://example.com/a.bin");
        let name = b64_encode(b"a.bin");
        let xml = format!(
            "<dlc><content><packages><package name=\"Wrapped\">\
             <file><url>{url}</url><filename>{name}</filename><size>10</size></file>\
             </package></packages></content></dlc>"
        );
        let result = import_dlc(xml.as_bytes()).unwrap();
        assert_eq!(result.len(), 1, "only the real <package> must be emitted");
        assert_eq!(result[0].name, "Wrapped");
        assert_eq!(result[0].links.len(), 1);
        assert_eq!(result[0].links[0].url, "https://example.com/a.bin");
        assert_eq!(result[0].links[0].filename.as_deref(), Some("a.bin"));
    }

    // Regression for #35: a filename that fails Base64 decoding must be dropped
    // (with a warning) while the link's URL is still imported — not silently
    // discarding the whole file.
    #[test]
    fn test_bad_filename_is_dropped_but_url_kept() {
        let url = b64_encode(b"https://example.com/b.bin");
        let xml = format!(
            "<dlc><content><package name=\"P\">\
             <file><url>{url}</url><filename>!!!not-base64!!!</filename></file>\
             </package></content></dlc>"
        );
        let result = import_dlc(xml.as_bytes()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].links.len(), 1);
        assert_eq!(result[0].links[0].url, "https://example.com/b.bin");
        assert_eq!(result[0].links[0].filename, None);
    }

    // Files outside any <package> are collected into a synthesized "Default".
    #[test]
    fn test_flat_file_list_without_package() {
        let url = b64_encode(b"https://example.com/flat.bin");
        let xml = format!("<dlc><content><file><url>{url}</url></file></content></dlc>");
        let result = import_dlc(xml.as_bytes()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Default");
        assert_eq!(result[0].links[0].url, "https://example.com/flat.bin");
    }
}
