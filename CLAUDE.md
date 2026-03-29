# CLAUDE.md — amigo-downloader

## Projekt-Vision

**amigo-downloader** ist ein performanter, modularer Download-Manager in Rust mit TypeScript-Plugin-System (QuickJS + SWC), responsiver Web-UI und nativen Apps via Tauri. Ziel: das beste Download-Tool das es gibt — schneller als JDownloader, leichter als pyLoad, erweiterbar für HTTP, Usenet und Torrent.

Organisation: `amigo-labs` auf GitHub.

---

## Architektur-Übersicht

```
┌─────────────────────────────────────────────────────────┐
│                    Distribution Layer                     │
│  ┌──────────┐  ┌───────────────┐  ┌─────┐  ┌─────────┐ │
│  │  Docker   │  │ Tauri Desktop │  │ CLI │  │  Tauri  │ │
│  │ (server/) │  │  (tauri/)     │  │     │  │ Mobile  │ │
│  └────┬──────┘  └──────┬────────┘  └──┬──┘  └────┬────┘ │
│       │                │              │           │      │
│       └────────┬───────┴──────┬───────┘           │      │
│                │              │                    │      │
│         ┌──────▼──────┐ ┌────▼─────┐              │      │
│         │  web-ui     │ │  Axum    │◄─────────────┘      │
│         │ (Svelte)    │ │  Server  │                     │
│         └─────────────┘ └────┬─────┘                     │
│                              │                           │
├──────────────────────────────┼───────────────────────────┤
│                         Core Layer                       │
│                              │                           │
│  ┌───────────────────────────▼─────────────────────────┐ │
│  │                    core crate                        │ │
│  │                                                     │ │
│  │  ┌─────────────┐ ┌────────────┐ ┌────────────────┐  │ │
│  │  │  Download    │ │  Chunk     │ │  Bandwidth     │  │ │
│  │  │  Coordinator │ │  Manager   │ │  Scheduler     │  │ │
│  │  └──────┬───────┘ └─────┬──────┘ └───────┬────────┘  │ │
│  │         │               │                │           │ │
│  │  ┌──────▼───────────────▼────────────────▼────────┐  │ │
│  │  │              Protocol Backends                  │  │ │
│  │  │  ┌────────┐  ┌─────────┐  ┌──────┐  ┌───────┐ │  │ │
│  │  │  │  HTTP/S │  │  Usenet │  │  HLS │  │ DASH  │ │  │ │
│  │  │  │(reqwest)│  │ (NNTP)  │  │      │  │       │ │  │ │
│  │  │  └────────┘  └─────────┘  └──────┘  └───────┘ │  │ │
│  │  └────────────────────────────────────────────────┘  │ │
│  │                                                     │ │
│  │  ┌──────────────┐ ┌─────────────┐ ┌──────────────┐  │ │
│  │  │  Queue /     │ │  Retry &    │ │  Post-       │  │ │
│  │  │  Scheduler   │ │  Recovery   │ │  Processing  │  │ │
│  │  └──────────────┘ └─────────────┘ └──────────────┘  │ │
│  │                                                     │ │
│  │  ┌──────────────────────────────────────────────┐    │ │
│  │  │  Container (DLC Import/Export, CCF, RSDF)    │    │ │
│  │  └──────────────────────────────────────────────┘    │ │
│  └─────────────────────────────────────────────────────┘ │
│                              │                           │
│  ┌───────────────────────────▼─────────────────────────┐ │
│  │              extractors crate                        │ │
│  │                                                     │ │
│  │  ┌──────────────────┐ ┌──────────────┐              │ │
│  │  │  YouTube          │ │  HLS / DASH  │              │ │
│  │  │ (InnerTube, N-Ch) │ │  (Built-in)  │              │ │
│  │  └──────────────────┘ └──────────────┘              │ │
│  └─────────────────────────────────────────────────────┘ │
│                              │                           │
│  ┌───────────────────────────▼─────────────────────────┐ │
│  │              plugin-runtime crate                    │ │
│  │                                                     │ │
│  │  ┌─────────────┐ ┌──────────────┐ ┌──────────────┐  │ │
│  │  │ QuickJS VM  │ │ Host API     │ │ Plugin       │  │ │
│  │  │ (Sandbox)   │ │ (Functions)  │ │ Loader       │  │ │
│  │  └─────────────┘ └──────────────┘ └──────────────┘  │ │
│  │  ┌─────────────┐ ┌──────────────┐ ┌──────────────┐  │ │
│  │  │ TS→JS       │ │ Registry     │ │ Updater      │  │ │
│  │  │ Transpiler  │ │ (Marketplace)│ │              │  │ │
│  │  └─────────────┘ └──────────────┘ └──────────────┘  │ │
│  └─────────────────────────────────────────────────────┘ │
│                              │                           │
│  ┌───────────────────────────▼─────────────────────────┐ │
│  │                  Storage Layer                       │ │
│  │  SQLite (Metadaten, Queue, History, Plugin-Config)   │ │
│  │  Filesystem (Downloads, Chunk-Temp, Plugin-Dateien)  │ │
│  └─────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

---

## Repository-Struktur (Monorepo)

```
amigo-downloader/
├── CLAUDE.md
├── Cargo.toml                   # Workspace root
├── crates/
│   ├── core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── coordinator.rs
│   │       ├── chunk.rs
│   │       ├── bandwidth.rs
│   │       ├── queue.rs
│   │       ├── retry.rs
│   │       ├── postprocess.rs
│   │       ├── container.rs     # DLC Import/Export
│   │       ├── i18n.rs          # Internationalization
│   │       ├── updater.rs       # Self-update logic
│   │       ├── update_events.rs # Update event broadcasting
│   │       ├── protocol/
│   │       │   ├── mod.rs
│   │       │   ├── http.rs
│   │       │   ├── hls.rs       # HLS manifest + segments
│   │       │   ├── dash.rs      # DASH/MPD manifest + segments
│   │       │   └── usenet/      # NNTP client, NZB, yEnc
│   │       ├── storage.rs
│   │       └── config.rs
│   ├── extractors/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── error.rs
│   │       ├── traits.rs        # Extractor trait, MediaStream types
│   │       └── youtube/
│   │           ├── mod.rs
│   │           ├── formats.rs       # Format/quality selection
│   │           ├── innertube.rs     # InnerTube API client
│   │           ├── n_challenge.rs   # N-parameter via QuickJS
│   │           └── url_parser.rs    # YouTube URL parsing
│   ├── plugin-runtime/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── engine.rs        # QuickJS VM execution
│   │       ├── host_api.rs
│   │       ├── loader.rs
│   │       ├── registry.rs      # Plugin marketplace
│   │       ├── sandbox.rs
│   │       ├── transpiler.rs    # TypeScript → JS via SWC
│   │       ├── types.rs
│   │       └── updater.rs       # Plugin auto-updates
│   ├── server/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── api.rs
│   │       ├── ws.rs
│   │       ├── clicknload.rs    # Click'n'Load on port 9666
│   │       ├── feedback.rs      # Bug/crash reporting
│   │       ├── update_api.rs    # Self-update endpoints
│   │       └── static_files.rs
│   └── cli/
│       ├── Cargo.toml
│       └── src/
│           └── main.rs
├── web-ui/
│   ├── package.json
│   ├── vite.config.ts
│   └── src/
│       ├── App.svelte
│       ├── app.css
│       ├── main.ts
│       ├── lib/
│       │   ├── api.ts
│       │   ├── stores.ts
│       │   └── toast.ts
│       ├── components/
│       │   ├── AddDialog.svelte
│       │   ├── ChunkViz.svelte
│       │   ├── DownloadCard.svelte
│       │   ├── DownloadRow.svelte
│       │   ├── DropZone.svelte
│       │   ├── FeedbackDialog.svelte
│       │   ├── Mascot.svelte
│       │   ├── Sparkline.svelte
│       │   └── Toasts.svelte
│       └── pages/
│           ├── Downloads.svelte
│           ├── History.svelte
│           ├── Plugins.svelte
│           ├── Queue.svelte
│           └── Settings.svelte
├── tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/
│       └── main.rs
├── plugins/
│   ├── README.md
│   ├── template/
│   │   └── plugin.ts
│   ├── types/
│   │   └── amigo.d.ts           # TypeScript type definitions
│   ├── hosters/
│   │   └── generic-http/
│   └── extractors/
│       └── youtube/
├── locales/
│   ├── en.json
│   └── de.json
├── scripts/
│   └── install.sh
├── docker/
│   ├── Dockerfile
│   └── docker-compose.yml
├── tests/
│   ├── integration/
│   └── plugins/
└── docs/
    ├── plugin-api.md
    ├── architecture.md
    ├── plan-youtube-hls-dash.md
    └── protocol-backends.md
```

---

## Tech Stack

| Komponente | Technologie | Begründung |
|---|---|---|
| Sprache Core | **Rust (2024 edition)** | Performance, Safety, async I/O |
| Async Runtime | **Tokio** | De-facto Standard |
| HTTP Client | **reqwest** | Connection Pooling, Cookie-Handling |
| HLS/DASH | **m3u8-rs, dash-mpd** | Streaming-Manifest-Parsing |
| Usenet/NNTP | **Eigene Impl auf Tokio** | Kein brauchbares Rust-Crate |
| Extractors | **Eigenes Crate + rquickjs** | YouTube N-Parameter, Format-Auswahl |
| Plugin Runtime | **QuickJS (rquickjs)** | Sandboxed JS VM, schnell |
| Plugin Sprache | **TypeScript via SWC** | DX, Typsicherheit, Transpilation zur Ladezeit |
| Datenbank | **SQLite via rusqlite** | Embedded, kein externer Service |
| Web Framework | **Axum** | Tokio-nativ, performant |
| Web-UI | **Svelte 5 + Tailwind v4 + Vite** | Kleine Bundles, reaktiv |
| Desktop/Mobile | **Tauri v2** | Rust-Backend, natives Window |
| CLI | **clap** | Standard für Rust CLIs |
| Serialisierung | **serde + serde_json** | Standard |
| Logging | **tracing** | Structured, async-aware |
| Archiv-Handling | **sevenz-rust, unrar, flate2** | Entpacken nach Download |
| DLC Container | **AES-128-CBC + Base64** | DLC Import/Export |
| i18n | **Eigene Impl, JSON Locales** | en, de out of the box |

---

## Core-Features

### Download-Engine (`core`)

#### HTTP/HTTPS
- Multi-Chunk parallel Downloads mit konfigurierbarer Chunk-Größe
- Automatisches Resume bei Verbindungsabbruch
- Connection Pooling pro Host
- Redirect-Following mit Loop-Detection
- Cookie-Jar pro Session und pro Plugin
- Custom Header-Support (Referer, User-Agent Rotation)
- HTTPS mit nativer TLS (rustls)
- Streaming-Downloads für unbekannte Dateigröße

#### BitTorrent
- Magnetlink- und .torrent-Datei-Support
- DHT, PEX, Selektiver Datei-Download
- Seeding mit konfigurierbarem Ratio-Limit
- Encryption (PE/MSE), IPv6, UPnP/NAT-PMP

#### Usenet (NNTP/NNTPS)
- SSL/TLS, Multi-Server mit Prioritäten
- Multi-Connection pro Server (10-50)
- yEnc-Decoding, NZB-Import
- PAR2-Verify/Repair, Auto-Unrar

#### DLC Container (Import & Export)
- **Import**: DLC-Dateien entschlüsseln und enthaltene Links extrahieren
  - AES-128-CBC Entschlüsselung mit bekanntem Key-Schema
  - Base64-Decoding der verschlüsselten Payload
  - XML-Parsing der entschlüsselten Link-Liste
  - Automatische Zuordnung zu Hoster-Plugins
- **Export**: Download-Listen als DLC-Datei exportieren
  - Links in DLC-XML-Format serialisieren
  - AES-128-CBC Verschlüsselung
  - Base64-Encoding für Transport
  - Kompatibel mit JDownloader und anderen Tools
- **CCF/RSDF**: Weitere Container-Formate (legacy, niedrigere Priorität)

#### Chunk-Manager
- Dynamische Chunk-Größe, Hash-Verification
- Memory-Mapped I/O Reassembly
- Temp-Dateien, Cleanup bei Abbruch

#### Bandwidth-Scheduler
- Global, Per-Download, Per-Protokoll Limits
- Zeitbasierte Schedules

#### Queue & Scheduler
- Priority-basierte Queue, Paket-Gruppen
- Max-gleichzeitige-Downloads konfigurierbar
- Pause/Resume auf Paket- und Einzeldownload-Ebene

#### Post-Processing Pipeline
- Auto-Entpacken: RAR, 7z, ZIP, tar.gz
- PAR2-Repair, CRC/Hash-Verification
- Zielverzeichnis-Regeln, Cleanup
- Custom Post-Processing via Plugins

#### Retry & Recovery
- Exponential Backoff mit Jitter
- Mirror-Failover, Usenet Backup-Server Fallback
- Partial-Download Recovery nach Crash

---

## Plugin-System (`plugin-runtime`)

Plugins sind TypeScript-Dateien (`.ts`), die zur Ladezeit via SWC nach JavaScript transpiliert und in einer sandboxed QuickJS VM ausgeführt werden.

### Host-API

```typescript
// Netzwerk
async function http_get(url: string, headers?: Record<string, string>): Promise<Response>;
async function http_post(url: string, body: string, content_type: string): Promise<Response>;
async function http_head(url: string): Promise<Response>;

// Cookie Management
function set_cookie(domain: string, name: string, value: string): void;
function get_cookie(domain: string, name: string): string | null;
function clear_cookies(domain: string): void;

// Parsing Helpers
function regex_match(pattern: string, text: string): string | null;
function regex_match_all(pattern: string, text: string): string[];
function html_select(html: string, css_selector: string): string[];
function html_attr(element: string, attr: string): string | null;
function json_parse(text: string): any;
function base64_decode(input: string): string;
function base64_encode(input: string): string;

// Crypto
function aes_decrypt(data: string, key: string, iv: string): string;
function md5(input: string): string;
function sha256(input: string): string;

// Logging, Storage, Captcha, Notifications
function log_info(msg: string): void;
function storage_get(key: string): string | null;
function storage_set(key: string, value: string): void;
async function captcha_solve_image(image_url: string): Promise<string>;
function notify(title: string, message: string): void;
function set_filename(name: string): void;
function set_filesize(bytes: number): void;
function set_wait(seconds: number): void;
```

### Plugin-Interface

```typescript
// REQUIRED
export function plugin_id(): string;
export function plugin_name(): string;
export function plugin_version(): string;
export function url_pattern(): string;
export async function resolve(url: string): Promise<DownloadInfo>;

// OPTIONAL
export function supports_premium(): boolean;
export async function login(username: string, password: string): Promise<boolean>;
export async function decrypt_container(data: string): Promise<string[]>;
export async function resolve_folder(url: string): Promise<string[]>;
export async function check_online(url: string): Promise<OnlineStatus>;
```

Type definitions for IDE support: `plugins/types/amigo.d.ts`

### Plugin Registry & Updates
- Marketplace: Plugins aus der Registry installieren/suchen
- Auto-Updates: Checksum-Verifikation, automatische Plugin-Updates
- Transpiler: TypeScript → JavaScript via SWC zur Ladezeit

### Sandboxing
- Kein direkter Netzwerk/Filesystem/Prozess-Zugang
- Resource Limits: 30s Timeout, 64MB RAM, 20 HTTP-Requests, 1MB Storage
- Hot-Reload via Filesystem-Watcher

---

## REST API

```
GET    /api/v1/status
GET    /api/v1/stats

POST   /api/v1/downloads
GET    /api/v1/downloads
GET    /api/v1/downloads/:id
PATCH  /api/v1/downloads/:id
DELETE /api/v1/downloads/:id

POST   /api/v1/downloads/batch
POST   /api/v1/downloads/nzb
POST   /api/v1/downloads/torrent
POST   /api/v1/downloads/container       # DLC/CCF Import
GET    /api/v1/downloads/export/dlc       # DLC Export

GET    /api/v1/queue
PATCH  /api/v1/queue/reorder
POST   /api/v1/queue/packages

GET    /api/v1/plugins
PATCH  /api/v1/plugins/:id
POST   /api/v1/plugins/:id/login

GET    /api/v1/usenet/servers
POST   /api/v1/usenet/servers
DELETE /api/v1/usenet/servers/:id

GET    /api/v1/torrent/:id/peers
GET    /api/v1/torrent/:id/trackers

GET    /api/v1/history
DELETE /api/v1/history

GET    /api/v1/config
PATCH  /api/v1/config

WS     /api/v1/ws
```

---

## CLI

```bash
# Direct download — just pass URLs (like yt-dlp)
amigo-dl <URL> [URL...]
amigo-dl <URL> -o ./downloads --chunks 8

# Queue-based (adds to DB, used with server)
amigo-dl add <URL>
amigo-dl add --nzb <file.nzb>
amigo-dl add --torrent <file.torrent>
amigo-dl add --magnet "<magnet:?...>"
amigo-dl add --dlc <file.dlc>              # DLC Import
amigo-dl export-dlc [--ids <id1,id2,...>]   # DLC Export
amigo-dl list / pause / resume / cancel / queue / status / speed
amigo-dl config get/set <key> <value>
amigo-dl plugins list / enable / login
amigo-dl serve [--port 8080 --bind 0.0.0.0]
```

---

## Coding-Konventionen

- **Language**: All code, comments, commit messages, variable names, docs — **English only**
- **Rust**: `cargo fmt` + `cargo clippy` (deny warnings), Rust 2024 Edition
- **Error Handling**: `thiserror` für Library-Errors, `anyhow` nur in Binaries
- **Async**: Alles async wo I/O involviert. Keine `block_on` im Core.
- **Tests**: Unit-Tests inline, Integration-Tests in `tests/`
- **Svelte**: TypeScript strict, Tailwind CSS v4
- **Git**: Conventional Commits (`feat:`, `fix:`, `refactor:`, `docs:`)
- **CI**: `cargo test`, `cargo clippy`, `npm run check`, Docker Build
- **Spec-Driven Development**: Every feature starts with a spec in `docs/specs/` before implementation

### Spec-Driven Development Workflow

Use the project skills to enforce consistency:

| Skill | Usage | Purpose |
|-------|-------|---------|
| `/spec <feature>` | Before coding | Write structured spec with testable acceptance criteria |
| `/verify` | After coding | Run 7-point consistency check (types, API, tests, i18n, docs) |
| `/feature <name>` | Full workflow | Orchestrate: Spec → Types → Tests → Backend → Frontend → Verify → Commit |

**Rules:**
1. No feature without a spec in `docs/specs/<feature>.md`
2. Every acceptance criterion must be testable
3. API contract (Rust types) defined before implementation
4. `/verify` must pass before merging

---

## Wichtige Design-Entscheidungen

1. **Monorepo** bis Plugin-API stabil (1.0), dann Plugins auslagern
2. **Svelte** statt React — kleinere Bundles für Tauri
3. **QuickJS + TypeScript** statt Rune/WASM/Lua — bessere DX, SWC-Transpilation, sandboxed
4. **SQLite** — embedded, kein externer Service
5. **Axum** — leichtgewichtig, Tokio-nativ
6. **reqwest** — Ergonomie, Cookie-Handling
7. **Single Binary** — Web-UI eingebettet via rust-embed
8. **Plugin-Sandbox** — kein direkter Netzwerk/FS-Zugriff
9. **DLC Import/Export** — Kompatibilität mit bestehendem Ökosystem
10. **Click'n'Load** auf Port 9666 — Browser-Extension Ökosystem nutzen
11. **Eigenes Extractor-Crate** — YouTube, HLS, DASH als Built-in Extractors
12. **i18n** — JSON-basierte Locale-Dateien (en, de)
