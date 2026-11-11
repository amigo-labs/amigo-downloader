Most settings are on the Settings page of the Web UI. They are stored in `config.toml`, which the server writes on first start; you can edit the file too, then restart the server.

## The file

An excerpt with the settings you are most likely to change. Edit them in the file the server generated; do not replace that file with this excerpt, it has more sections than shown here.

```toml
download_dir = "downloads"
temp_dir = "downloads/.tmp"
max_concurrent_downloads = 10

[http]
max_chunks_per_download = 8
max_connections_per_host = 4
timeout_connect_secs = 30
timeout_read_secs = 120

[bandwidth]
global_limit = 0          # bytes per second, 0 = unlimited
schedule_enabled = true

[[bandwidth.schedules]]
name = "Night"
start = "01:00"
end = "07:00"
limit = 0                 # unlimited

[[bandwidth.schedules]]
name = "Day"
start = "07:00"
end = "01:00"
limit = 5000000           # 5 MB/s

[postprocessing]
auto_extract = true       # unpack RAR, ZIP, 7z, tar.gz
delete_archives = true

[retry]
max_retries = 5
base_delay_secs = 1.0
max_delay_secs = 60.0

[update]
auto_check = true
check_interval_hours = 24
auto_update_plugins = false

[features]
usenet = false            # NZB import, NNTP servers, watch folder
rss_feeds = false
```

## Speed limits

`global_limit` caps all downloads together. With `schedule_enabled`, the entry whose time span contains the current time sets the limit instead, for example unlimited at night and 5 MB/s during the day. The sidebar of the Web UI has a quick switch for a temporary limit.

## Usenet

Usenet is off by default. Turn on `features.usenet` (or the switch on the Settings page), then add your NNTP servers there. Krake imports NZB files, decodes yEnc, and verifies and repairs with PAR2.

## Plugins

Plugins are TypeScript files. The server loads the ones in its `plugins` folder at start; the Plugins page installs and updates them from the registry. Each plugin runs sandboxed in WebAssembly and reaches the network only through Krake. A plugin that declares its hosts can reach only those, and installing it shows what it asks for; one without a declaration is marked "Unscoped".

To write one, start from [`plugins/template/plugin.ts`](https://github.com/amigo-labs/amigo-krake/blob/main/plugins/template/plugin.ts) and read the [plugin API](https://github.com/amigo-labs/amigo-krake/blob/main/docs/plugin-api.md).

## Other tools

- **Browser extensions:** Click'n'Load listens on port 9666, like JDownloader, for browsers on the same machine as the server (not with the Docker image).
- **NZBGet clients:** the server answers NZBGet's JSON-RPC at `/jsonrpc`.
- **Scripts:** a REST API under `/api/v1/` and a WebSocket at `/api/v1/ws` for live progress.
