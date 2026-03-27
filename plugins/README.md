# amigo-downloader Plugins

Rune-based plugin system for hoster support.

## Creating a Plugin

1. Copy `plugin-template/plugin.rn` to `hosters/your_hoster.rn`
2. Implement the required functions: `plugin_id`, `plugin_name`, `plugin_version`, `url_pattern`, `resolve`
3. Place the file in the `plugins/hosters/` directory — it will be auto-detected

See `plugin-template/plugin.rn` for the full template.
