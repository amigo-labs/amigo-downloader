//! HTTP/HTTPS download backend via reqwest.

pub struct HttpDownloader {
    client: reqwest::Client,
}

impl HttpDownloader {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub async fn download(&self, _url: &str, _path: &str) -> Result<(), crate::Error> {
        todo!("HTTP download with chunking, resume, progress reporting")
    }

    pub async fn head(&self, _url: &str) -> Result<HeadInfo, crate::Error> {
        todo!("HEAD request to get file size and resume support")
    }
}

pub struct HeadInfo {
    pub content_length: Option<u64>,
    pub accepts_ranges: bool,
    pub filename: Option<String>,
}
