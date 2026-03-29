<div align="center">
  <img src="web-ui/public/icon-512.svg" width="128" height="128" alt="amigo-downloader mascot" />

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

#### Server & updates

```bash
amigo-dl serve                         # start REST API (port 1516)
amigo-dl serve --port 9090 --bind 127.0.0.1
amigo-dl update check                  # check for new version
amigo-dl update apply --yes            # apply update
```

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
├── plugins/               # TypeScript plugin files (.ts)
│   ├── extractors/        # Site-specific extractors (YouTube)
│   ├── hosters/           # Hoster plugins (generic-http)
│   ├── template/          # Plugin template
│   └── types/             # TypeScript type definitions (amigo.d.ts)
├── tauri/                 # Tauri v2 desktop app
├── locales/               # i18n translation files (en, de)
├── scripts/               # Install script
├── docker/                # Dockerfile + compose
└── docs/                  # Architecture docs
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

Plugins are TypeScript files (`.ts`) that run in a sandboxed QuickJS VM. TypeScript is transpiled to JavaScript at load time via SWC.

```typescript
export function plugin_id() { return "my-hoster"; }
export function plugin_name() { return "My Hoster"; }
export function plugin_version() { return "1.0.0"; }
export function url_pattern() { return "https?://my-hoster\\.com/.+"; }

export async function resolve(url: string): Promise<DownloadInfo> {
    const html = await http_get(url);
    const link = regex_match(/href="(https:\/\/dl\..+?)"/, html.body);
    return { url: link, filename: "file.bin", chunks_supported: true };
}
```

**Sandbox limits:** 30s timeout, 64MB RAM, 20 HTTP requests, 1MB storage per plugin. No direct network/filesystem access — everything proxied through the Host API.

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
- **In-app feedback**: Report bugs directly from the settings page

### Pages

| Page | Description |
|------|-------------|
| Downloads | Active/completed downloads with progress |
| Queue | Priority queue management |
| Plugins | Plugin list, install, update, configure |
| History | Past downloads with search/filter |
| Settings | Config, theming, bandwidth schedules |

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

## Plugin Development

1. Copy `plugins/template/plugin.ts`
2. Implement `plugin_id`, `plugin_name`, `plugin_version`, `url_pattern`, `resolve`
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
