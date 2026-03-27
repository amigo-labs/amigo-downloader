//! BitTorrent download backend.

pub struct TorrentDownloader;

impl TorrentDownloader {
    pub fn new() -> Self {
        Self
    }

    pub async fn download_torrent(&self, _torrent_path: &str, _output_dir: &str) -> Result<(), crate::Error> {
        todo!("BitTorrent download with DHT, PEX, encryption")
    }

    pub async fn download_magnet(&self, _magnet_uri: &str, _output_dir: &str) -> Result<(), crate::Error> {
        todo!("Magnet link → metadata → download")
    }
}
