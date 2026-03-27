# CLAUDE.md — amigo-downloader

## Projekt-Vision

**amigo-downloader** ist ein performanter, modularer Download-Manager in Rust mit Rune-Plugin-System, responsiver Web-UI und nativen Apps via Tauri. Ziel: das beste Download-Tool das es gibt — schneller als JDownloader, leichter als pyLoad, erweiterbar für HTTP, Usenet und Torrent.

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
│  │  │  ┌────────┐  ┌─────────┐  ┌──────────────────┐ │  │ │
│  │  │  │  HTTP/S │  │  Usenet │  │  BitTorrent      │ │  │ │
│  │  │  │(reqwest)│  │ (NNTP)  │  │(libtorrent/own)  │ │  │ │
│  │  │  └────────┘  └─────────┘  └──────────────────┘ │  │ │
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
│  │              plugin-runtime crate                    │ │
│  │                                                     │ │
│  │  ┌─────────────┐ ┌──────────────┐ ┌──────────────┐  │ │
│  │  │ Rune VM     │ │ Host API     │ │ Plugin       │  │ │
│  │  │ (Sandbox)   │ │ (Functions)  │ │ Loader       │  │ │
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
│   │       ├── container.rs     # DLC Import/Export, CCF, RSDF
│   │       ├── protocol/
│   │       │   ├── mod.rs
│   │       │   ├── http.rs
│   │       │   ├── usenet.rs
│   │       │   └── torrent.rs
│   │       ├── storage.rs
│   │       └── config.rs
│   ├── plugin-runtime/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── host_api.rs
│   │       ├── loader.rs
│   │       ├── sandbox.rs
│   │       └── types.rs
│   ├── server/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── api.rs
│   │       ├── ws.rs
│   │       └── static_files.rs
│   └── cli/
│       ├── Cargo.toml
│       └── src/
│           └── main.rs
├── web-ui/
│   ├── package.json
│   ├── svelte.config.js
│   ├── vite.config.ts
│   └── src/
│       ├── App.svelte
│       ├── lib/
│       │   ├── api.ts
│       │   └── stores.ts
│       └── routes/
│           ├── downloads/
│           ├── queue/
│           ├── plugins/
│           ├── usenet/
│           ├── torrent/
│           ├── history/
│           └── settings/
├── tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── src/
│   │   └── main.rs
│   ├── icons/
│   └── capabilities/
├── plugins/
│   ├── README.md
│   ├── plugin-template/
│   │   └── plugin.rn
│   └── hosters/
│       ├── generic_http.rn
│       └── ...
├── docker/
│   ├── Dockerfile
│   └── docker-compose.yml
├── tests/
│   ├── integration/
│   └── plugins/
└── docs/
    ├── plugin-api.md
    ├── architecture.md
    └── protocol-backends.md
```

---

## Tech Stack

| Komponente | Technologie | Begründung |
|---|---|---|
| Sprache Core | **Rust (latest stable)** | Performance, Safety, async I/O |
| Async Runtime | **Tokio** | De-facto Standard |
| HTTP Client | **reqwest** | Connection Pooling, Cookie-Handling |
| BitTorrent | **librqbit** oder eigene Impl | Pure-Rust, async-native |
| Usenet/NNTP | **Eigene Impl auf Tokio** | Kein brauchbares Rust-Crate |
| Plugin Runtime | **Rune** | Rust-native, async, sandboxed |
| Datenbank | **SQLite via rusqlite** | Embedded, kein externer Service |
| Web Framework | **Axum** | Tokio-nativ, performant |
| Web-UI | **Svelte 5 + Vite** | Kleine Bundles, reaktiv |
| Desktop/Mobile | **Tauri v2** | Rust-Backend, natives Window |
| CLI | **clap** | Standard für Rust CLIs |
| Serialisierung | **serde + serde_json** | Standard |
| Logging | **tracing** | Structured, async-aware |
| Archiv-Handling | **sevenz-rust, unrar, flate2** | Entpacken nach Download |
| DLC Container | **AES-128-CBC + Base64** | DLC Import/Export |

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

### Host-API

```rust
// Netzwerk
async fn http_get(url: String, headers: Option<Object>) -> Result<Response>;
async fn http_post(url: String, body: String, content_type: String) -> Result<Response>;
async fn http_head(url: String) -> Result<Response>;

// Cookie Management
fn set_cookie(domain: String, name: String, value: String);
fn get_cookie(domain: String, name: String) -> Option<String>;
fn clear_cookies(domain: String);

// Parsing Helpers
fn regex_match(pattern: String, text: String) -> Option<String>;
fn regex_match_all(pattern: String, text: String) -> Vec<String>;
fn html_select(html: String, css_selector: String) -> Vec<String>;
fn html_attr(element: String, attr: String) -> Option<String>;
fn json_parse(text: String) -> Value;
fn base64_decode(input: String) -> String;
fn base64_encode(input: String) -> String;

// Crypto
fn aes_decrypt(data: String, key: String, iv: String) -> Result<String>;
fn md5(input: String) -> String;
fn sha256(input: String) -> String;

// Logging, Storage, Captcha, Notifications
fn log_info(msg: String);
fn storage_get(key: String) -> Option<String>;
fn storage_set(key: String, value: String);
async fn captcha_solve_image(image_url: String) -> Result<String>;
fn notify(title: String, message: String);
fn set_filename(name: String);
fn set_filesize(bytes: u64);
fn set_wait(seconds: u64);
```

### Plugin-Interface

```rust
// REQUIRED
pub fn plugin_id() -> String;
pub fn plugin_name() -> String;
pub fn plugin_version() -> String;
pub fn url_pattern() -> String;
pub async fn resolve(url: String) -> Result<DownloadInfo>;

// OPTIONAL
pub fn supports_premium() -> bool;
pub async fn login(username: String, password: String) -> Result<bool>;
pub async fn decrypt_container(data: String) -> Result<Vec<String>>;
pub async fn resolve_folder(url: String) -> Result<Vec<String>>;
pub async fn check_online(url: String) -> Result<OnlineStatus>;
```

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

- **Rust**: `cargo fmt` + `cargo clippy` (deny warnings), Rust 2024 Edition
- **Error Handling**: `thiserror` für Library-Errors, `anyhow` nur in Binaries
- **Async**: Alles async wo I/O involviert. Keine `block_on` im Core.
- **Tests**: Unit-Tests inline, Integration-Tests in `tests/`
- **Svelte**: TypeScript strict, Prettier, ESLint
- **Git**: Conventional Commits (`feat:`, `fix:`, `refactor:`, `docs:`)
- **CI**: `cargo test`, `cargo clippy`, `npm run check`, Docker Build

---

## Wichtige Design-Entscheidungen

1. **Monorepo** bis Plugin-API stabil (1.0), dann Plugins auslagern
2. **Svelte** statt React — kleinere Bundles für Tauri
3. **Rune** statt WASM/Lua — native async + Rust-Integration
4. **SQLite** — embedded, kein externer Service
5. **Axum** — leichtgewichtig, Tokio-nativ
6. **reqwest** — Ergonomie, Cookie-Handling
7. **Single Binary** — Web-UI eingebettet via rust-embed
8. **Plugin-Sandbox** — kein direkter Netzwerk/FS-Zugriff
9. **DLC Import/Export** — Kompatibilität mit bestehendem Ökosystem
10. **Click'n'Load** auf Port 9666 — Browser-Extension Ökosystem nutzen
