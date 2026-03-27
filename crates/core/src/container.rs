//! DLC, CCF, and RSDF container import/export.
//!
//! DLC (Download Link Container) uses AES-128-CBC encryption with Base64 encoding.
//! This module handles both importing (decrypting) and exporting (encrypting) DLC files.

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

/// Import a DLC file: decrypt and extract contained links.
pub fn import_dlc(_data: &[u8]) -> Result<Vec<ContainerPackage>, crate::Error> {
    todo!("AES-128-CBC decrypt → Base64 decode → XML parse → extract links")
}

/// Export download links as a DLC file (compatible with JDownloader etc.).
pub fn export_dlc(_packages: &[ContainerPackage]) -> Result<Vec<u8>, crate::Error> {
    todo!("Serialize links → XML → AES-128-CBC encrypt → Base64 encode")
}

/// Import a CCF container (legacy format).
pub fn import_ccf(_data: &[u8]) -> Result<Vec<ContainerPackage>, crate::Error> {
    todo!("CCF container import")
}

/// Import an RSDF container (legacy format).
pub fn import_rsdf(_data: &[u8]) -> Result<Vec<ContainerPackage>, crate::Error> {
    todo!("RSDF container import")
}
