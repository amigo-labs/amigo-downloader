//! Usenet/NNTP download backend.

pub struct UsenetDownloader;

impl UsenetDownloader {
    pub fn new() -> Self {
        Self
    }

    pub async fn download_nzb(&self, _nzb_path: &str, _output_dir: &str) -> Result<(), crate::Error> {
        todo!("Parse NZB, download articles via NNTP, yEnc decode, PAR2 verify/repair")
    }
}
