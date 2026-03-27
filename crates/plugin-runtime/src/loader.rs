//! Plugin discovery, loading, and hot-reload.

use std::path::Path;

use crate::types::PluginMeta;

pub struct PluginLoader {
    plugin_dir: String,
}

impl PluginLoader {
    pub fn new(plugin_dir: String) -> Self {
        Self { plugin_dir }
    }

    /// Scan plugin directory and load all .rn files.
    pub async fn discover(&self) -> Result<Vec<PluginMeta>, crate::Error> {
        todo!("Scan plugin_dir for .rn files, validate, extract metadata")
    }

    /// Find a plugin that matches the given URL.
    pub fn match_url(&self, _url: &str) -> Option<PluginMeta> {
        todo!("Match URL against loaded plugin url_patterns")
    }

    /// Watch plugin directory for changes and hot-reload.
    pub async fn watch(&self, _path: &Path) -> Result<(), crate::Error> {
        todo!("Filesystem watcher for hot-reload")
    }
}
