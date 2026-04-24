<div align="center">
  <img src="amigo-downloader.png" width="128" height="128" alt="amigo-downloader mascot" />

  <h1>amigo-downloader</h1>

  <p><strong>Fast, modular download manager</strong> built in Rust with a responsive Web UI, plugin system, and native apps.</p>
  <p>Extensible for HTTP, Usenet, and more.</p>

  <p>
    <a href="https://github.com/amigo-labs/amigo-downloader/actions"><img src="https://img.shields.io/github/actions/workflow/status/amigo-labs/amigo-downloader/ci.yml?branch=main&style=flat-square&logo=github&label=CI" alt="CI" /></a>
    <a href="https://github.com/amigo-labs/amigo-downloader/releases"><img src="https://img.shields.io/github/v/release/amigo-labs/amigo-downloader?style=flat-square&logo=rust&label=Release" alt="Release" /></a>
    <a href="https://github.com/amigo-labs/amigo-downloader/blob/main/LICENSE"><img src="https://img.shields.io/github/license/amigo-labs/amigo-downloader?style=flat-square" alt="License" /></a>
  </p>
</div>

---

## Features

- **Multi-chunk parallel downloads** with automatic resume and retry
- **Built-in extractors** — YouTube (with N-parameter challenge), HLS, DASH
- **TypeScript plugin system** — scriptable hoster support, hot-reloadable `.ts` files
- **Plugin marketplace** — install and update plugins from the registry
- **Responsive Web UI** — Svelte 5 + Tailwind, dark/light themes, 6 accent colors
- **PWA with Share Target** — install on your phone, share links directly to add downloads
- **DLC container** import and export (JDownloader compatible)
- **Usenet support** — NZB parsing, NNTP/SSL multi-connection, yEnc decoding
- **Click'n'Load** on port 9666 — works with browser extensions
- **REST API** + **WebSocket** for real-time progress
- **Internationalization** — English and German out of the box
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

To build from source locally instead of pulling the image:

```bash
docker compose -f docker/docker-compose.local.yml up -d --build
```

Open `http://localhost:1516` in your browser.

### From source

```bash
# Build web UI
cd web-ui && npm ci && npx vite build && cd ..

# Build and run server
cargo run --release --bin amigo-server
```

### CLI

```bash
# Install to ~/.cargo/bin/
cargo install --path crates/cli

# Use it
amigo-dl https://example.com/file.zip
```

<details>
<summary><strong>CLI reference</strong></summary>

#### Direct download

Just pass URLs — downloads directly with a progress bar, like yt-dlp.

```bash
amigo-dl <URL> [URL...]
amigo-dl <URL> -o ./downloads          # custom output directory
amigo-dl <URL> --chunks 8              # force 8 parallel chunks
```

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--output` | `-o` | `.` | Output directory |
| `--chunks` | `-c` | `0` (auto) | Chunks per download, 0 = auto based on file size |

#### Queue management

For use with the server/web UI — adds downloads to the database.

```bash
amigo-dl add <URL>                     # queue a URL
amigo-dl add --nzb file.nzb            # import NZB
amigo-dl add --dlc links.dlc           # import DLC container
amigo-dl list                          # list active downloads
amigo-dl pause <ID>                    # pause a download
amigo-dl resume <ID>                   # resume a download
amigo-dl cancel <ID>                   # cancel and remove
amigo-dl queue                         # show queue
amigo-dl status                        # overview (active, queued, speed)
amigo-dl speed                         # current download speed
```

#### DLC containers

```bash
amigo-dl add --dlc links.dlc           # import
amigo-dl export-dlc                    # export all
amigo-dl export-dlc --ids id1,id2      # export specific
```

#### Configuration

```bash
amigo-dl config get <key>              # read a config value
amigo-dl config set <key> <value>      # write a config value
```

#### Plugins

```bash
amigo-dl plugins list                  # list installed plugins
amigo-dl plugins install <id>          # install from registry
amigo-dl plugins update [id]           # update plugins
amigo-dl plugins search <query>        # search registry
amigo-dl plugins enable <id>           # enable a plugin
amigo-dl plugins login <id>            # login to a hoster account
```

#### Updates

```bash
amigo-dl update check                  # check for a new version
amigo-dl update apply --yes            # apply the update
```

Run `amigo-server` (or the Docker image) to get the full daemon with
Web UI — the CLI is a client, not a daemon host.

</details>

## Architecture

```
amigo-downloader/
├── crates/
│   ├── core/              # Download engine, protocols, storage, i18n
│   ├── extractors/        # Built-in site extractors (YouTube, HLS, DASH)
│   ├── plugin-runtime/    # QuickJS VM, TypeScript transpiler, plugin registry
│   ├── server/            # Axum REST API, WebSocket, Click'n'Load, embedded UI
│   └── cli/               # Command-line interface
├── web-ui/                # Svelte 5 + Tailwind PWA
├── plugin-sdk/            # @amigo/plugin-sdk — TS SDK for plugin authors
├── plugins/               # TypeScript plugin files (.ts)
│   ├── extractors/        # Site-specific extractors (youtube)
│   ├── hosters/           # Hoster plugins (generic-http, xfilesharing)
│   ├── multi-hosters/     # Premium aggregators (alldebrid, premiumize, real-debrid)
│   ├── template/          # Plugin template
│   └── types/             # TypeScript type definitions (amigo.d.ts)
├── tauri/                 # Tauri v2 desktop app
├── locales/               # i18n translation files (en, de)
├── scripts/               # Install script
├── docker/                # Dockerfile(s) + compose files
└── docs/                  # Architecture docs + specs
```

### Core Crate

| Module | Purpose |
|--------|---------|
| `coordinator` | Orchestrates downloads, pause/resume/cancel, auto-start |
| `protocol/http` | Multi-chunk parallel HTTP/HTTPS downloads, resume, progress |
| `protocol/hls` | HLS manifest parsing and segment downloading |
| `protocol/dash` | DASH/MPD manifest parsing and segment downloading |
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
| `update_events` | Update event broadcasting |
| `i18n` | Internationalization support |

### Extractors Crate

Built-in extractors for sites that need special handling beyond simple HTTP:

| Module | Purpose |
|--------|---------|
| `youtube` | YouTube video extraction with format selection |
| `youtube/n_challenge` | N-parameter deobfuscation via QuickJS |
| `youtube/innertube` | YouTube InnerTube API client |
| `youtube/formats` | Format/quality selection logic |
| `youtube/url_parser` | YouTube URL parsing (video, playlist, shorts) |

### Plugin System

Plugins are TypeScript files (`.ts`) that run in a sandboxed QuickJS VM. TypeScript is transpiled to JavaScript at load time via SWC. A plugin exports an object via `module.exports`; all Host-API calls are **synchronous** (the runtime blocks internally) — no `async`/`await` inside plugin code.

```typescript
/// <reference path="../types/amigo.d.ts" />

module.exports = {
    id: "my-hoster",
    name: "My Hoster",
    version: "1.0.0",
    urlPattern: "https?://my-hoster\\.com/.+",

    resolve(url: string): DownloadPackage {
        const page = amigo.httpGet(url);
        const link = amigo.htmlQueryAttr(page.body, "a.download-btn", "href");
        return {
            name: amigo.htmlExtractTitle(page.body) || "Download",
            downloads: [{
                url: amigo.urlResolve(url, link!),
                filename: null, filesize: null,
                chunks_supported: true, max_chunks: null,
                headers: null, cookies: null, wait_seconds: null,
                mirrors: [],
            }],
        };
    },
} satisfies AmigoPlugin;
```

**Sandbox limits:** 30s timeout, 64MB RAM, 20 HTTP requests, 1MB storage per plugin. No direct network/filesystem access — everything proxied through the `amigo.*` Host API.

The plugin runtime includes:
- **Registry** — discover and install plugins from the marketplace
- **Updater** — automatic plugin updates with checksum verification
- **Transpiler** — TypeScript → JavaScript via SWC at load time
- **Engine** — QuickJS VM execution with Host API bindings

### Server

| Module | Purpose |
|--------|---------|
| `api` | REST API routes for downloads, queue, plugins, config |
| `ws` | WebSocket endpoint for real-time progress events |
| `clicknload` | Click'n'Load listener on port 9666 |
| `feedback` | In-app bug/crash reporting to GitHub Issues |
| `update_api` | Self-update API endpoints |
| `static_files` | Embedded Web UI via rust-embed |

### REST API

```
GET    /api/v1/status              Server status + version
GET    /api/v1/stats               Speed, queue size, active count
GET    /api/v1/system-info         System information

POST   /api/v1/downloads           Add download (URL)
GET    /api/v1/downloads           List all downloads
GET    /api/v1/downloads/{id}      Single download
PATCH  /api/v1/downloads/{id}      Pause/resume
DELETE /api/v1/downloads/{id}      Cancel + remove
POST   /api/v1/downloads/batch     Add multiple URLs
POST   /api/v1/downloads/nzb       Upload NZB file
POST   /api/v1/downloads/container Import DLC (multipart, field "file")
GET    /api/v1/downloads/usenet    List Usenet downloads

GET    /api/v1/queue               View queue
PATCH  /api/v1/queue/reorder       Reorder queue

GET    /api/v1/history             Download history
DELETE /api/v1/history             Clear history

GET    /api/v1/usenet/servers      List NNTP servers
POST   /api/v1/usenet/servers      Add NNTP server
DELETE /api/v1/usenet/servers/{id} Delete NNTP server
GET    /api/v1/usenet/watch-dir    Get NZB watch directory
POST   /api/v1/usenet/watch-dir    Set NZB watch directory

GET    /api/v1/plugins             List plugins
PATCH  /api/v1/plugins/{id}        Update plugin config
POST   /api/v1/plugins/suggest     Suggest a plugin for a URL

GET    /api/v1/captcha/pending     Pending captcha requests
POST   /api/v1/captcha/{id}/solve  Submit captcha solution
POST   /api/v1/captcha/{id}/cancel Cancel captcha request

GET    /api/v1/webhooks            List outbound webhooks
POST   /api/v1/webhooks            Create webhook
DELETE /api/v1/webhooks/{id}       Delete webhook
POST   /api/v1/webhooks/{id}/test  Fire a test event

GET    /api/v1/rss                 List RSS feeds
POST   /api/v1/rss                 Add RSS feed
DELETE /api/v1/rss/{id}            Delete RSS feed

GET    /api/v1/config              Read full config (TOML)
PUT    /api/v1/config              Replace full config

GET    /api/v1/updates/check       Check for core + plugin updates
POST   /api/v1/updates/core        Apply core self-update
POST   /api/v1/updates/plugins/{id}         Update a plugin
POST   /api/v1/updates/plugins/{id}/install Install a plugin

POST   /api/v1/feedback            Submit bug/crash report
WS     /api/v1/ws                  Live progress events

# NZBGet JSON-RPC compatibility (root path, not /api/v1)
POST   /jsonrpc
POST   /{username}/jsonrpc
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
- **In-app feedback**: Report bugs directly from the settings page

### Pages

| Page | Description |
|------|-------------|
| Downloads | Active/completed downloads with progress; side panel for details |
| History | Past downloads with search/filter |
| Plugins | Plugin list, install, update, configure |
| Usenet Servers | NNTP server configuration |
| Settings | Config, theming, bandwidth schedules, feedback |

## Mobile / PWA

1. Open `http://your-server:1516` on your phone
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
      - "1516:1516"   # Web UI
      - "9666:9666"   # Click'n'Load
    volumes:
      - ./config:/config
      - ./downloads:/downloads
      - ./plugins:/etc/amigo/plugins     # Custom plugins
    environment:
      - AMIGO_GITHUB_TOKEN=ghp_...  # Optional: auto crash reporting
```

To build from source locally, use the local compose file:

```bash
docker compose -f docker/docker-compose.local.yml up -d --build
```

## Plugin Development

1. Copy `plugins/template/plugin.ts`
2. Set `id`, `name`, `version`, `urlPattern` and implement `resolve()` on the exported object
3. Drop the `.ts` file into `plugins/hosters/` or `plugins/extractors/`
4. It's auto-detected and loaded (hot-reload supported)

Type definitions are available in `plugins/types/amigo.d.ts` for IDE support.

See [Plugin API docs](docs/plugin-api.md) for the full Host API reference.

## Tech Stack

| Component | Technology |
|-----------|-----------|
| Core | Rust (2024 edition), Tokio async runtime |
| HTTP | reqwest with connection pooling |
| HLS/DASH | m3u8-rs, dash-mpd |
| Usenet | Custom NNTP/TLS client on Tokio |
| Extractors | Built-in (YouTube via rquickjs for N-parameter) |
| Plugins | QuickJS VM (sandboxed), TypeScript via SWC transpiler |
| Database | SQLite (rusqlite, WAL mode) |
| Web API | Axum + WebSocket |
| Web UI | Svelte 5, Tailwind CSS v4, Vite |
| Desktop | Tauri v2 |
| CLI | clap |
| i18n | Custom, JSON locale files |

## Contributing

```bash
# Run tests
cargo test --workspace

# Check + clippy
cargo check --workspace
cargo clippy --workspace

# Build web UI
cd web-ui && npm ci && npx vite build

# Type-check web UI
cd web-ui && npm run check
```

## License

MIT
