<div align="center">
  <img src="web-ui/public/icon-512.svg" width="128" height="128" alt="amigo-downloader mascot" />

  <h1>amigo-downloader</h1>

  <p><strong>Fast, modular download manager</strong> built in Rust with a responsive Web UI, plugin system, and native apps.</p>
  <p>Faster than JDownloader, lighter than pyLoad, extensible for HTTP, Usenet, and more.</p>

  <p>
    <img src="https://img.shields.io/badge/Rust-2024_edition-orange?style=flat-square&logo=rust" alt="Rust" />
    <img src="https://img.shields.io/badge/Svelte-5-ff3e00?style=flat-square&logo=svelte" alt="Svelte 5" />
    <img src="https://img.shields.io/badge/Docker-ready-blue?style=flat-square&logo=docker" alt="Docker" />
    <img src="https://img.shields.io/badge/License-MIT-green?style=flat-square" alt="License" />
    <img src="https://img.shields.io/badge/PWA-Share_Target-blueviolet?style=flat-square&logo=pwa" alt="PWA" />
  </p>
</div>

---

## Features

- **Multi-chunk parallel downloads** with automatic resume and retry
- **Rune plugin system** — scriptable hoster support, hot-reloadable `.rn` files
- **Responsive Web UI** — Svelte 5 + Tailwind, dark/light themes, 6 accent colors
- **PWA with Share Target** — install on your phone, share links directly to add downloads
- **DLC container** import and export (JDownloader compatible)
- **Usenet support** — NZB parsing, NNTP/SSL multi-connection, yEnc decoding
- **Click'n'Load** on port 9666 — works with browser extensions
- **REST API** + **WebSocket** for real-time progress
- **Plugin marketplace** — install and update plugins from the registry
- **Self-update** — core binary auto-updates from GitHub Releases
- **In-app feedback** — crash auto-reporting to GitHub Issues (with dedup)
- **Post-processing** — auto-extract RAR, ZIP, 7z, tar.gz after download
- **Bandwidth scheduler** — time-based speed limits (e.g. unlimited at night)
- **Crash recovery** — interrupted downloads resume on restart
- **Single binary** — Web UI embedded, no external dependencies
- **Docker ready** — multi-stage build, config via environment variables

## Quick Start

### Docker (recommended)

```bash
docker compose -f docker/docker-compose.yml up -d
```

Open `http://localhost:8080` in your browser.

### From source

```bash
# Build web UI
cd web-ui && npm ci && npx vite build && cd ..

# Build and run server
cargo run --release --bin amigo-server
```

### CLI

```bash
cargo run --release --bin amigo -- add https://example.com/file.zip
cargo run --release --bin amigo -- list
cargo run --release --bin amigo -- add --nzb file.nzb
cargo run --release --bin amigo -- add --dlc links.dlc
```

## Architecture

```
amigo-downloader/
├── crates/
│   ├── core/              # Download engine, protocols, storage
│   ├── plugin-runtime/    # Rune VM, sandbox, plugin registry
│   ├── server/            # Axum REST API, WebSocket, embedded UI
│   └── cli/               # Command-line interface
├── web-ui/                # Svelte 5 + Tailwind PWA
├── plugins/               # Rune plugin files (.rn)
├── docker/                # Dockerfile + compose
└── docs/                  # Architecture docs
```

### Core Crate

| Module | Purpose |
|--------|---------|
| `coordinator` | Orchestrates downloads, pause/resume/cancel, auto-start |
| `protocol/http` | Multi-chunk parallel downloads, resume, progress |
| `protocol/usenet` | NNTP client, NZB parser, yEnc decoder |
| `chunk` | Chunk splitting, reassembly, verification |
| `bandwidth` | Token bucket limiter with time-based schedules |
| `queue` | Priority queue with status tracking |
| `storage` | SQLite with WAL mode, full CRUD |
| `container` | DLC import/export (Base64 + XML) |
| `postprocess` | Auto-extract archives after download |
| `retry` | Exponential backoff with jitter |
| `config` | TOML file loading/saving, env var support |
| `updater` | Self-update from GitHub Releases |

### Plugin System

Plugins are `.rn` scripts (Rune language) that run in a sandboxed VM:

```rust
pub fn plugin_id() { "my-hoster" }
pub fn plugin_name() { "My Hoster" }
pub fn plugin_version() { "1.0.0" }
pub fn url_pattern() { r"https?://my-hoster\.com/.+" }

pub async fn resolve(url) {
    let html = http_get(url, None).await?;
    let link = regex_match(r#"href="(https://dl\..+?)""#, html.body)?;
    Ok(#{ url: link, filename: "file.bin", chunks_supported: true })
}
```

**Sandbox limits:** 30s timeout, 64MB RAM, 20 HTTP requests, 1MB storage per plugin. No direct network/filesystem access — everything proxied through the Host API.

### REST API

```
GET    /api/v1/status              Server status + version
GET    /api/v1/stats               Speed, queue size, active count
POST   /api/v1/downloads           Add download (URL)
GET    /api/v1/downloads           List all downloads
PATCH  /api/v1/downloads/:id       Pause/resume
DELETE /api/v1/downloads/:id       Cancel + remove
POST   /api/v1/downloads/batch     Add multiple URLs
POST   /api/v1/downloads/nzb       Upload NZB file
GET    /api/v1/queue               View queue
GET    /api/v1/history             Download history
GET    /api/v1/plugins             List plugins
GET    /api/v1/updates/check       Check for updates
POST   /api/v1/feedback            Submit bug/crash report
GET    /api/v1/system-info         System information
WS     /api/v1/ws                  Live progress events
```

## Web UI

The web interface is a Svelte 5 PWA with:

- **Dual layout**: Modern (cards) or Classic (table) — your choice
- **Theming**: Dark/Light mode + 6 accent colors (Blue, Green, Purple, Coral, Orange, Cyan)
- **Pixel-art mascot**: Animated "Amigo" robot — bounces when downloading
- **Speed sparkline**: Mini graph showing speed history in the sidebar
- **Chunk visualization**: See parallel chunks downloading in real-time
- **Toast notifications**: Slide-in alerts for completed/failed downloads
- **Drag & drop**: Drop URLs, NZB, DLC files anywhere on the page
- **Keyboard shortcuts**: `Ctrl+N` add download, `1-5` navigate, `Esc` close
- **PWA Share Target**: Install on phone, share links from any app

## Mobile / PWA

1. Open `http://your-server:8080` on your phone
2. "Add to Home Screen"
3. Share any URL from any app to "Amigo"

That's it. The link is added as a download instantly.

## Configuration

Config file: `config.toml` (auto-created on first start)

```toml
[general]
download_dir = "downloads"
max_concurrent_downloads = 10

[http]
max_chunks_per_download = 8
user_agent = "amigo-downloader/0.1.0"

[bandwidth]
global_limit = 0  # bytes/s, 0 = unlimited
schedule_enabled = true

[[bandwidth.schedules]]
name = "Night"
start = "01:00"
end = "07:00"
limit = 0  # unlimited

[[bandwidth.schedules]]
name = "Day"
start = "07:00"
end = "01:00"
limit = 5000000  # 5 MB/s

[feedback]
github_token = ""  # or set AMIGO_GITHUB_TOKEN env var
github_repo = "amigo-labs/amigo-downloader"
```

## Docker

```yaml
services:
  amigo-downloader:
    image: ghcr.io/amigo-labs/amigo-downloader:latest
    ports:
      - "8080:8080"   # Web UI
      - "9666:9666"   # Click'n'Load
    volumes:
      - ./config:/config
      - ./downloads:/downloads
    environment:
      - AMIGO_GITHUB_TOKEN=ghp_...  # Optional: auto crash reporting
```

## Plugin Development

1. Copy `plugins/plugin-template/plugin.rn`
2. Implement `plugin_id`, `plugin_name`, `plugin_version`, `url_pattern`, `resolve`
3. Drop the `.rn` file into `plugins/hosters/`
4. It's auto-detected and loaded (hot-reload supported)

See [Plugin API docs](docs/plugin-api.md) for the full Host API reference.

## Tech Stack

| Component | Technology |
|-----------|-----------|
| Core | Rust (2024 edition), Tokio async runtime |
| HTTP | reqwest with connection pooling |
| Usenet | Custom NNTP/TLS client on Tokio |
| Plugins | Rune VM (sandboxed, async) |
| Database | SQLite (rusqlite, WAL mode) |
| Web API | Axum + WebSocket |
| Web UI | Svelte 5, Tailwind CSS v4, Vite |
| Desktop | Tauri v2 (planned) |
| CLI | clap |

## Contributing

```bash
# Run tests
cargo test --workspace

# Check + clippy
cargo check --workspace
cargo clippy --workspace

# Build web UI
cd web-ui && npm ci && npx vite build
```

## License

MIT
