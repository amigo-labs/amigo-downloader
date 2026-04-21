# CLAUDE.md — amigo-downloader

## Projekt-Vision

**amigo-downloader** ist ein performanter, modularer Download-Manager in Rust mit TypeScript-Plugin-System (QuickJS + SWC), responsiver Web-UI und nativen Apps via Tauri. Ziel: das beste Download-Tool das es gibt — schneller als JDownloader, leichter als pyLoad, mit Built-in-Support für HTTP, Usenet, HLS und DASH.

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
├── README.md
├── LICENSE
├── Cargo.toml                   # Workspace root
├── Cargo.lock
├── Cross.toml                   # cross-rs build config
├── release-please-config.json
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
│   │       ├── captcha.rs       # Captcha manager (pending requests)
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
│   │       ├── host_api.rs      # amigo.* host functions
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
│   │       ├── lib.rs
│   │       ├── api.rs           # REST API routes
│   │       ├── background.rs    # Background tasks (RSS, etc.)
│   │       ├── ws.rs            # WebSocket endpoint
│   │       ├── clicknload.rs    # Click'n'Load on port 9666
│   │       ├── nzbget_api.rs    # NZBGet JSON-RPC compat layer
│   │       ├── resolver.rs      # URL → plugin resolver
│   │       ├── webhooks.rs      # Outbound webhook dispatcher
│   │       ├── feedback.rs      # Bug/crash reporting
│   │       ├── update_api.rs    # Self-update endpoints
│   │       └── static_files.rs  # Embedded Web UI
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
│       │   ├── AddPanel.svelte
│       │   ├── CaptchaDialog.svelte
│       │   ├── ChunkViz.svelte
│       │   ├── ContextMenu.svelte
│       │   ├── DetailPanel.svelte
│       │   ├── DownloadCard.svelte
│       │   ├── DownloadCompactRow.svelte
│       │   ├── DropZone.svelte
│       │   ├── FeedbackDialog.svelte
│       │   ├── Icon.svelte
│       │   ├── Mascot.svelte
│       │   ├── ProgressRing.svelte
│       │   ├── ShortcutsDialog.svelte
│       │   ├── SidePanel.svelte
│       │   ├── SkeletonCard.svelte
│       │   ├── Sparkline.svelte
│       │   ├── Toasts.svelte
│       │   └── settings/         # Settings subpanels
│       └── pages/
│           ├── Downloads.svelte
│           ├── History.svelte
│           ├── Plugins.svelte
│           ├── Settings.svelte
│           └── UsenetServers.svelte
├── tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/
│       └── main.rs
├── plugin-sdk/                   # @amigo/plugin-sdk — TS SDK for plugin authors
│   ├── package.json
│   ├── tsconfig.json
│   ├── vitest.config.ts
│   ├── src/
│   ├── test/
│   └── docs/
│       ├── cookbook.md
│       ├── jdownloader-mapping.md
│       └── tutorial.md
├── plugins/
│   ├── README.md
│   ├── template/
│   │   └── plugin.ts
│   ├── types/
│   │   └── amigo.d.ts           # TypeScript type definitions
│   ├── hosters/
│   │   ├── generic-http/
│   │   └── xfilesharing/
│   ├── multi-hosters/           # Premium aggregator plugins
│   │   ├── alldebrid/
│   │   ├── premiumize/
│   │   └── real-debrid/
│   └── extractors/
│       └── youtube/
├── locales/
│   ├── en.json
│   └── de.json
├── scripts/
│   └── install.sh
├── docker/
│   ├── Dockerfile
│   ├── Dockerfile.dev
│   ├── docker-compose.yml       # Pulls the published image
│   ├── docker-compose.local.yml # Builds from source
│   └── docker-compose.dev.yml
├── tests/
│   ├── integration/
│   └── plugins/
└── docs/
    ├── plugin-api.md
    ├── architecture.md
    ├── plan-youtube-hls-dash.md
    ├── plan-plugin-sdk.md
    ├── protocol-backends.md
    └── specs/                    # Spec-driven development
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

#### HLS & DASH
- HLS-Manifest-Parsing (m3u8-rs) inkl. Varianten- und Audio-Track-Auswahl
- DASH/MPD-Manifest-Parsing (dash-mpd), Segment-Downloads
- Nutzbar sowohl von Built-in-Extractors (YouTube) als auch aus Plugins via `protocol: "hls" | "dash"` im `DownloadInfo`

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

Die autoritative Plugin-API-Referenz ist `docs/plugin-api.md`; die vollständigen TypeScript-Typen liegen in `plugins/types/amigo.d.ts`. Dieser Abschnitt ist nur eine Zusammenfassung.

### Plugin-Interface

Ein Plugin exportiert ein Objekt über `module.exports` (CommonJS-Stil, QuickJS ist synchron):

```typescript
/// <reference path="../types/amigo.d.ts" />

module.exports = {
    // Required
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

    // Optional
    description: "...", author: "...",
    pluginType: "hoster",                 // "multi-hoster" | "hoster" | "generic"
    checkOnline(url): "online" | "offline" | "unknown" { /* ... */ },
    login(username, password): boolean { /* ... */ },
    supportsPremium(): boolean { /* ... */ },
    decryptContainer(data): string[] { /* ... */ },
    resolveFolder(url): string[] { /* ... */ },
    postProcess(context): PostProcessResult { /* ... */ },
} satisfies AmigoPlugin;
```

Wichtig: `resolve()` und alle Plugin-Callbacks sind **synchron**. Die Host-API blockiert intern (via `spawn_blocking` in der Runtime). Kein `await`, kein `async`.

### Host-API (`amigo.*`)

Alle Funktionen sind unter dem globalen `amigo`-Objekt verfügbar. camelCase-Namen, synchron, gehen durch die Sandbox.

```typescript
// HTTP (keine direkten Netz-Calls)
amigo.httpGet(url, opts?): HttpResponse
amigo.httpPost(url, body, contentType, opts?): HttpResponse
amigo.httpHead(url, opts?): HeadResponse
amigo.httpGetJson(url, opts?): HttpJsonResponse
amigo.httpPostForm(url, fields, opts?): HttpResponse
amigo.httpGetBinary(url, opts?): string          // base64
amigo.httpFollowRedirects(url, opts?): string

// Cookies
amigo.setCookie(domain, name, value): void
amigo.getCookie(domain, name): string | null
amigo.clearCookies(domain): void

// URL helpers
amigo.urlResolve(base, relative): string
amigo.urlParse(url): ParsedUrl
amigo.urlFilename(url): string | null

// HTML helpers (scraper crate)
amigo.htmlQueryAll(html, selector): string[]
amigo.htmlQueryText(html, selector): string | null
amigo.htmlQueryAttr(html, selector, attr): string | null
amigo.htmlQueryAllAttrs(html, selector, attr): string[]
amigo.htmlSearchMeta(html, names): string | null
amigo.htmlExtractTitle(html): string | null
amigo.htmlHiddenInputs(html): Record<string, string>
amigo.searchJson(startPattern, html): any | null

// Regex
amigo.regexMatch(pattern, text): string | null
amigo.regexMatchAll(pattern, text): string[]
amigo.regexReplace(pattern, text, replacement): string | null
amigo.regexTest(pattern, text): boolean
amigo.regexSplit(pattern, text): string[]

// Encoding & crypto
amigo.base64Encode(input): string
amigo.base64Decode(input): string
amigo.md5(input): string                         // hex
amigo.sha1(input): string                        // hex
amigo.sha256(input): string                      // hex
amigo.hmacSha256(key, data): string              // hex
amigo.aesEncryptCbc(data, key, iv): string       // base64 in/out, hex key/iv
amigo.aesDecryptCbc(data, key, iv): string

// Utility
amigo.parseDuration(input): number | null        // "1:23:45" / "PT2H30M" → seconds
amigo.sanitizeFilename(name): string
amigo.traverse(obj, path): any | null            // safe deep access

// Captcha (blocks until solved in Web UI, throws on timeout/skip)
amigo.solveCaptcha(imageUrl, type?): string      // "image" | "recaptcha" | "hcaptcha"

// Notifications, logging, per-plugin storage
amigo.notify(title, message): void
amigo.logInfo / logWarn / logError / logDebug(msg): void
amigo.storageGet(key): string | null
amigo.storageSet(key, value): void
amigo.storageDelete(key): void
```

Ebenfalls verfügbar: `console.log` / `console.warn` / `console.error`.

Dateiname/Dateigröße/Wait werden **nicht** über Setter-Funktionen gemeldet — sie gehören ins zurückgegebene `DownloadInfo` (`filename`, `filesize`, `wait_seconds`).

### TypeScript-SDK (`plugin-sdk/`)

Für komplexere Plugins existiert das separate Package `@amigo/plugin-sdk` mit höheren Abstraktionen (Browser, Page, Form, CookieJar, Captcha-Helpers, HLS/DASH-Manifest-Parser, Container-Helpers). Der Plan ist in `docs/plan-plugin-sdk.md` dokumentiert, Tutorials in `plugin-sdk/docs/`.

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

Alle Routen unter `/api/v1/`. DLC-Import per HTTP geht über `POST /downloads/container` (multipart, Feld `file`); DLC-Export läuft aktuell nur über die CLI (`amigo-dl export-dlc`).

```
# Server status
GET    /status
GET    /stats
GET    /system-info

# Downloads
POST   /downloads                         # single URL
GET    /downloads
GET    /downloads/{id}
PATCH  /downloads/{id}                    # pause / resume
DELETE /downloads/{id}                    # cancel + remove
POST   /downloads/batch                   # multiple URLs
POST   /downloads/nzb                     # upload NZB
POST   /downloads/container               # import DLC (multipart, field "file")
GET    /downloads/usenet                  # list usenet downloads only

# Queue & history
GET    /queue
PATCH  /queue/reorder
GET    /history
DELETE /history

# Usenet servers
GET    /usenet/servers
POST   /usenet/servers
DELETE /usenet/servers/{id}
GET    /usenet/watch-dir
POST   /usenet/watch-dir

# Plugins
GET    /plugins
PATCH  /plugins/{id}
POST   /plugins/suggest                   # suggest a plugin for a URL

# Captcha (manual solving via Web UI)
GET    /captcha/pending
POST   /captcha/{id}/solve
POST   /captcha/{id}/cancel

# Webhooks
GET    /webhooks
POST   /webhooks
DELETE /webhooks/{id}
POST   /webhooks/{id}/test

# RSS feeds
GET    /rss
POST   /rss
DELETE /rss/{id}

# Self-update (core + plugins)
GET    /updates/check
POST   /updates/core
POST   /updates/plugins/{id}
POST   /updates/plugins/{id}/install

# Config (single unified TOML resource)
GET    /config
PUT    /config

# Feedback (in-app crash/bug reporting → GitHub Issues)
POST   /feedback

# Real-time events
WS     /ws

# NZBGet JSON-RPC compatibility (root path, not under /api/v1)
POST   /jsonrpc
POST   /{username}/jsonrpc
```

---

## CLI

Tatsächlich unterstützte Sub-Commands (aus `crates/cli/src/main.rs`):

```bash
# Direct download — bare URLs (like yt-dlp) or explicit `get`
amigo-dl <URL> [URL...]
amigo-dl get <URL> -o ./downloads --chunks 8

# Queue-based (requires server/DB)
amigo-dl add <URL>                          # queue a URL
amigo-dl add --nzb <file.nzb>               # import NZB
amigo-dl add --dlc <file.dlc>               # import DLC container
amigo-dl export-dlc [--ids <id1,id2,...>]   # export DLC container
amigo-dl list / pause <id> / resume <id> / cancel <id>
amigo-dl queue / status / speed

# Config
amigo-dl config get <key>
amigo-dl config set <key> <value>

# Plugins
amigo-dl plugins list
amigo-dl plugins enable <id>
amigo-dl plugins login <id>
amigo-dl plugins install <id>
amigo-dl plugins search <query>
amigo-dl plugins update [id]
amigo-dl plugins test <plugin.ts> [url]     # run spec or resolve a URL

# Self-update
amigo-dl update check
amigo-dl update apply [--yes]

# Server
amigo-dl serve [--port 1516 --bind 0.0.0.0]
```

Torrent/Magnet sind aktuell nicht implementiert. `crates/core/src/protocol/` enthält nur `http`, `hls`, `dash`, `usenet`.

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

| Skill | When | Purpose |
|-------|------|---------|
| `/spec <name>` | Before coding | Create or update a feature spec collaboratively |
| `/spec-extract <area>` | For existing code | Reverse-engineer a spec from what's already implemented |
| `/spec-implement <name>` | After spec | Implement the spec: Contract → Code → Verify+Commit |
| `/spec-verify [name]` | Anytime | Verify spec compliance and project consistency |
| `/spec-fix <bug>` | Bugfixes | Lightweight: find root cause → fix → regression test → commit |

**Rules:**
1. Features and significant changes need a spec in `docs/specs/<name>.md`
2. Bugfixes use `/spec-fix` — no spec needed unless the fix reveals a spec gap
3. Every acceptance criterion must be testable
4. `/spec-verify` must pass before merging

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

---

## Design System (Corporate Neon)

> Full design system in `.impeccable.md`. Summary below for every session.

### Target Audience
Power users and tech enthusiasts managing downloads (HTTP, Usenet) centrally. Dark mode preferred, status at a glance.

### Brand: Technical, Precise, Powerful
- Corporate Neon — professional at rest, electric on interaction
- 6 color palettes: Blue, Teal, Indigo, Amber, Violet, Rose
- Dark mode primary, "Lights On" as alternative
- Neon intensity 5 levels (Off to Full), CRT scanlines only at Full

### Design Principles
1. **Glow Follows Function** — Neon effects reinforce information hierarchy
2. **Instant Orientation** — Status, speed, progress at a glance
3. **Adjustable Intensity** — User decides cyberpunk level
4. **Responsive Without Compromise** — 320px to 2560px
5. **Accessibility** — `prefers-reduced-motion`, WCAG AA across all themes

### Tokens
- **Typography**: Inter + ui-monospace stack, Major Third Scale
- **Breakpoint**: 768px (single primary)
- **60:30:10**: 60% dark bg, 30% surfaces/text/borders, 10% neon accents
- **Neon Glow**: --neon-glow-sm/md/lg, --neon-text-glow, --neon-border (scaled by intensity)
- **Status**: online=#22c55e, warning=#eab308, error=#ef4444
