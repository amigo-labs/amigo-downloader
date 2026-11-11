# AGENTS.md — Krake

Instructions for every coding agent working in this repository (Claude Code reads them through `CLAUDE.md`, which only imports this file).

## Projekt-Vision

**Krake** ist ein performanter, modularer Download-Manager in Rust mit TypeScript-Plugin-System (QuickJS + SWC), responsiver Web-UI und nativen Apps via Tauri. Ziel: das beste Download-Tool das es gibt — schneller als JDownloader, leichter als pyLoad, mit Built-in-Support für HTTP, Usenet, HLS und DASH.

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
amigo-krake/
├── AGENTS.md                    # This file (CLAUDE.md only imports it)
├── CLAUDE.md
├── .agents/                     # Agent skills (mirrored to .claude/skills), gates.json, sync.mjs
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
│   │       ├── engine.rs        # Engine config, shared resolve/postProcess/spec scripts
│   │       ├── host_api.rs      # krake.* host functions (daemon side)
│   │       ├── host_fns.rs      # Host-function table + JS shim
│   │       ├── ipc.rs           # Daemon ↔ plugin host wire protocol
│   │       ├── plugin_host.rs   # Child process that runs plugin JS
│   │       ├── process_host.rs  # Spawns/supervises the plugin host
│   │       ├── bin/krake-plugin-host.rs
│   │       ├── loader.rs
│   │       ├── registry.rs      # Plugin marketplace
│   │       ├── sandbox.rs
│   │       ├── transpiler.rs    # TypeScript → JS via SWC
│   │       ├── types.rs
│   │       ├── updater.rs       # Plugin auto-updates
│   │       └── wasm.rs          # Wasm executor: runs the plugin guest in wasmtime
│   ├── plugin-guest/            # QuickJS as WebAssembly (not a workspace member)
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
│           └── Settings.svelte
├── tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/
│       └── main.rs
├── plugin-sdk/                   # @amigo-labs/krake-plugin-sdk — TS SDK for plugin authors
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
│   │   └── krake.d.ts           # TypeScript type definitions
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
│   ├── install.sh
│   └── sign-update-archives.sh  # Ed25519 .sig for self-update archives
├── docker/
│   ├── Dockerfile
│   ├── Dockerfile.dev
│   ├── docker-compose.yml       # Pulls the published image
│   ├── docker-compose.local.yml # Builds from source
│   └── docker-compose.dev.yml
├── tests/
│   └── README.md                # Pointer: integration tests live in crates/*/tests/
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
| Sprache Core | **Rust 1.99.0 (2024 edition, pinned in `rust-toolchain.toml`)** | Performance, Safety, async I/O |
| Async Runtime | **Tokio** | De-facto Standard |
| HTTP Client | **reqwest** | Connection Pooling, Cookie-Handling |
| HLS/DASH | **m3u8-rs, dash-mpd** | Streaming-Manifest-Parsing |
| Usenet/NNTP | **Eigene Impl auf Tokio** | Kein brauchbares Rust-Crate |
| Extractors | **Eigenes Crate + rquickjs** | YouTube N-Parameter, Format-Auswahl |
| Plugin Runtime | **QuickJS als WebAssembly (rquickjs im Guest, wasmtime)** | Sandboxed JS VM, Speicherisolation pro Plugin |
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

Plugins sind TypeScript-Dateien (`.ts`), die zur Ladezeit via SWC nach JavaScript transpiliert und in einer sandboxed QuickJS VM ausgeführt werden (QuickJS als WebAssembly, eine Instanz pro Plugin).

Die autoritative Plugin-API-Referenz ist `docs/plugin-api.md`; die vollständigen TypeScript-Typen liegen in `plugins/types/krake.d.ts`. Dieser Abschnitt ist nur eine Zusammenfassung.

### Plugin-Interface

Ein Plugin exportiert ein Objekt über `module.exports` (CommonJS-Stil, QuickJS ist synchron):

```typescript
/// <reference path="../types/krake.d.ts" />

module.exports = {
    // Required
    id: "my-hoster",
    name: "My Hoster",
    version: "1.0.0",
    apiVersion: 1,                        // host-API major (HOST_API_VERSION)
    urlPattern: "https?://my-hoster\\.com/.+",

    resolve(url: string): DownloadPackage {
        const page = krake.httpGet(url);
        const link = krake.htmlQueryAttr(page.body, "a.download-btn", "href");
        return {
            name: krake.htmlExtractTitle(page.body) || "Download",
            downloads: [{
                url: krake.urlResolve(url, link!),
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
    permissions: { domains: ["my-hoster.com"] }, // hosts krake.http* may reach
    checkOnline(url): "online" | "offline" | "unknown" { /* ... */ },
    login(username, password): boolean { /* ... */ },
    supportsPremium(): boolean { /* ... */ },
    decryptContainer(data): string[] { /* ... */ },
    resolveFolder(url): string[] { /* ... */ },
    postProcess(context): PostProcessResult { /* ... */ },
} satisfies KrakePlugin;
```

Wichtig: `resolve()` und alle Plugin-Callbacks sind **synchron**. Host-Calls blockieren im Plugin-Host-Prozess, bis der Daemon antwortet. Kein `await`, kein `async`.

### Host-API (`krake.*`)

Alle Funktionen sind unter dem globalen `krake`-Objekt verfügbar. camelCase-Namen, synchron, gehen durch die Sandbox. Der alte Name `amigo` (und der Typ `AmigoPlugin`) ist ein deprecated Alias auf dasselbe Objekt, damit Plugins von vor der Umbenennung weiterlaufen.

```typescript
// HTTP (keine direkten Netz-Calls)
krake.httpGet(url, opts?): HttpResponse
krake.httpPost(url, body, contentType, opts?): HttpResponse
krake.httpHead(url, opts?): HeadResponse
krake.httpGetJson(url, opts?): HttpJsonResponse
krake.httpPostForm(url, fields, opts?): HttpResponse
krake.httpGetBinary(url, opts?): string          // base64
krake.httpFollowRedirects(url, opts?): string

// Cookies
krake.setCookie(domain, name, value): void
krake.getCookie(domain, name): string | null
krake.clearCookies(domain): void

// URL helpers
krake.urlResolve(base, relative): string
krake.urlParse(url): ParsedUrl
krake.urlFilename(url): string | null

// HTML helpers (scraper crate)
krake.htmlQueryAll(html, selector): string[]
krake.htmlQueryText(html, selector): string | null
krake.htmlQueryAttr(html, selector, attr): string | null
krake.htmlQueryAllAttrs(html, selector, attr): string[]
krake.htmlSearchMeta(html, names): string | null
krake.htmlExtractTitle(html): string | null
krake.htmlHiddenInputs(html): Record<string, string>
krake.searchJson(startPattern, html): any | null

// Regex
krake.regexMatch(pattern, text): string | null
krake.regexMatchAll(pattern, text): string[]
krake.regexReplace(pattern, text, replacement): string | null
krake.regexTest(pattern, text): boolean
krake.regexSplit(pattern, text): string[]

// Encoding & crypto
krake.base64Encode(input): string
krake.base64Decode(input): string
krake.md5(input): string                         // hex
krake.sha1(input): string                        // hex
krake.sha256(input): string                      // hex
krake.hmacSha256(key, data): string              // hex
krake.aesEncryptCbc(data, key, iv): string       // base64 in/out, hex key/iv
krake.aesDecryptCbc(data, key, iv): string

// Utility
krake.parseDuration(input): number | null        // "1:23:45" / "PT2H30M" → seconds
krake.sanitizeFilename(name): string
krake.traverse(obj, path): any | null            // safe deep access

// Captcha (blocks until solved in Web UI, throws on timeout/skip)
krake.solveCaptcha(imageUrl, type?): string      // "image" | "recaptcha" | "hcaptcha"

// Notifications, logging, per-plugin storage
krake.notify(title, message): void
krake.logInfo / logWarn / logError / logDebug(msg): void
krake.storageGet(key): string | null
krake.storageSet(key, value): void
krake.storageDelete(key): void
```

Ebenfalls verfügbar: `console.log` / `console.warn` / `console.error`.

Dateiname/Dateigröße/Wait werden **nicht** über Setter-Funktionen gemeldet — sie gehören ins zurückgegebene `DownloadInfo` (`filename`, `filesize`, `wait_seconds`).

### TypeScript-SDK (`plugin-sdk/`)

Für komplexere Plugins existiert das separate Package `@amigo-labs/krake-plugin-sdk` mit höheren Abstraktionen (Browser, Page, Form, CookieJar, Captcha-Helpers, HLS/DASH-Manifest-Parser, Container-Helpers). Der Plan ist in `docs/plan-plugin-sdk.md` dokumentiert, Tutorials in `plugin-sdk/docs/`.

### Plugin Registry & Updates
- Marketplace: Plugins aus der Registry installieren/suchen
- Auto-Updates: Checksum-Verifikation, automatische Plugin-Updates
- Transpiler: TypeScript → JavaScript via SWC zur Ladezeit

### Sandboxing
- Kein direkter Netzwerk/Filesystem/Prozess-Zugang
- Resource Limits: 30s Timeout (5s für Modul-Load), 64MB RAM pro Plugin (eigene Wasm-Instanz mit eigener QuickJS-Runtime pro Plugin), 20 HTTP-Requests, 1MB Storage
- Netzwerk: optionale Domain-Allowlist `permissions.domains`, pro Request und Redirect-Hop erzwungen; Install/erweiternde Updates brauchen Bestätigung
- Plugin-JS läuft im separaten Prozess `krake-plugin-host` (liegt neben `krake-server`/`krake`, Override: `KRAKE_PLUGIN_HOST_BIN`); dort läuft QuickJS als WebAssembly in `wasmtime` (`crates/plugin-guest`, `wasm.rs`): ein Speicherfehler in QuickJS bleibt in der Wasm-Instanz des Plugins. Reißt ein Aufruf ein Limit oder trappt die Instanz, schlägt nur dieser Aufruf fehl und der nächste bekommt eine frische Instanz; stürzt der Prozess ab, läuft die Queue weiter. Keine zusätzliche OS-Sandbox um den Child (Entscheidung 14, `docs/specs/plugin-wasm-sandbox.md`)
- Reload: Laden eines Plugins mit bekannter `id` ersetzt die alte Version (genutzt von Plugin-Updates); kein Filesystem-Watcher — nach manuellen Änderungen Server neu starten oder `krake plugins test` zum Iterieren nutzen

---

## REST API

Alle Routen unter `/api/v1/`. DLC-Import per HTTP geht über `POST /downloads/container` (multipart, Feld `file`); DLC-Export läuft aktuell nur über die CLI (`krake export-dlc`).

```
# Server status
GET    /status
GET    /stats
GET    /system-info

# Downloads
POST   /downloads                         # single URL
GET    /downloads
GET    /downloads/{id}
PATCH  /downloads/{id}                    # pause / resume / retry
DELETE /downloads/{id}                    # cancel + remove
POST   /downloads/batch                   # add multiple URLs
PATCH  /downloads/batch                   # bulk pause / resume / retry
DELETE /downloads/batch                   # bulk cancel + remove
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
krake <URL> [URL...]
krake get <URL> -o ./downloads --chunks 8

# Queue-based (requires server/DB)
krake add <URL>                          # queue a URL
krake add --nzb <file.nzb>               # import NZB
krake add --dlc <file.dlc>               # import DLC container
krake export-dlc [--ids <id1,id2,...>]   # export DLC container
krake list / pause <id> / resume <id> / cancel <id>
krake queue / status / speed

# Config
krake config get <key>
krake config set <key> <value>

# Plugins
krake plugins list
krake plugins enable <id>
krake plugins login <id>
krake plugins install <id>
krake plugins search <query>
krake plugins update [id]
krake plugins test <plugin.ts> [url]     # run spec or resolve a URL

# Self-update
krake update check
krake update apply [--yes]

# Remote pairing (CLI → remote krake-server / Docker)
krake login <url> [--name <alias>]
krake remote list|remove <name>|use <name>
```

The daemon lives in the separate `krake-server` binary (and the Docker
image that wraps it). `krake` is strictly a client + direct downloader.

Torrent/Magnet sind aktuell nicht implementiert. `crates/core/src/protocol/` enthält nur `http`, `hls`, `dash`, `usenet`.

---

## Coding-Konventionen

- **Language**: All code, comments, commit messages, variable names, docs — **English only**
- **Rust**: `cargo fmt` + `cargo clippy` (deny warnings), Rust 2024 Edition
- **Error Handling**: `thiserror` für Library-Errors, `anyhow` nur in Binaries
- **Async**: Alles async wo I/O involviert. Keine `block_on` im Core.
- **Tests**: Unit-Tests inline, Integration-Tests in `crates/*/tests/` (siehe `tests/README.md`)
- **Svelte**: TypeScript strict, Tailwind CSS v4
- **Git**: Conventional Commits (`feat:`, `fix:`, `refactor:`, `docs:`)
- **CI**: `cargo test`, `cargo clippy`, `npm run check`, Docker Build
- **Spec-Driven Development**: Every feature starts with a spec in `docs/specs/` before implementation

### Spec-Driven Development Workflow

Features are built with the shared spec workflow from [amigo-labs/skills](https://github.com/amigo-labs/skills): `spec` → `spec-build` → `spec-check`, bugs with `spec-fix`, the gates with `qa`. The rules are the managed block "Spec-driven workflow" at the end of this file; the project facts the skills rely on are in "Spec workflow profile" below. Skills live in `.agents/skills/` (every agent) and are mirrored to `.claude/skills/` for Claude Code; `node .agents/sync.mjs` updates them.

Two more shared skills are vendored for the web UI: `improve <target> [idea]` (measure a screen, flow or module with scripts, change it once, keep the change only if a blind old/new comparison wins in both orders) and `ui-polish` (screenshot-scored improvement loop). The facts `improve` needs are under **Improve** in the profile.

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
13. **Eine Akzentfarbe** — die Web-UI nutzt nur die Markenfarbe Limette `#9be22d` (`docs/site/project.yml`), keine wählbaren Paletten: die Farbe gehört zur Marke, nicht zu den Einstellungen
14. **Plugin-Isolation durch WebAssembly** — QuickJS läuft als Wasm-Modul (`wasm32-unknown-unknown`, ohne WASI) in `wasmtime` innerhalb von `krake-plugin-host`, statt einer OS-Sandbox pro Plattform (Landlock/seccomp, AppContainer, `sandbox_init`): ein Mechanismus für Windows, macOS und Linux, in CI testbar, und ein Speicherfehler in QuickJS bleibt im Wasm-Speicher. Der eigene Prozess bleibt als zweite Schicht; es gibt keinen nativen Ausweichweg. Neue Abhängigkeit `wasmtime`; der Build braucht `clang` mit Wasm-Target. Specs: `docs/specs/plugin-wasm-guest.md`, `docs/specs/plugin-wasm-sandbox.md` (#79)

---

## Design System (Corporate Neon)

> Full design system in `.impeccable.md`. Summary below for every session.

### Target Audience
Power users and tech enthusiasts managing downloads (HTTP, Usenet) centrally. Dark mode preferred, status at a glance.

### Brand: Technical, Precise, Powerful
- Corporate Neon — professional at rest, electric on interaction
- One accent: the brand lime `#9be22d` (no user palettes)
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

---

## Gotchas

Pitfalls that code and git history don't show. Short entries; `/spec-build` adds to this list when a loop surprised it.

- **The build needs `clang` with a WebAssembly target and the Rust target `wasm32-unknown-unknown`.** `crates/plugin-runtime/build.rs` builds `crates/plugin-guest` (QuickJS as Wasm) with a nested `cargo` and embeds it precompiled. The guest is not a workspace member: `cargo fmt --all`, clippy and `cargo test --workspace` do not see it; run `cargo fmt` inside `crates/plugin-guest`, and it has its own `Cargo.lock`. Apple's clang has no Wasm target (`brew install llvm`, then `CC_wasm32_unknown_unknown`).
- **A Wasm instance outliving its `WasmEngine` still needs the deadline ticker.** Instances hold the ticker alive themselves; a test that creates the engine as a temporary once hung forever in `while (true) {}` because nothing advanced the epoch.
- **`pkill -f <test name>` kills the shell that runs it**, because the pattern is in its own command line. Stop a hung test binary by PID.
- **The build needs `protoc`.** `cargo build` / `clippy` fail in `pssh-box`'s build script without it (`apt-get install protobuf-compiler`); CI installs it.
- **`vite dev` hides what the server enforces.** It sets no CSP header and proxies `/api`, so a blocked font or script only shows against the real server (`cargo run -p krake-server`). A debug server reads `web-ui/dist` from disk: rebuild the UI, no server restart.
- **The server refuses downloads from private addresses**, loopback included (`crates/server/src/net_guard.rs`, no setting). A local test server therefore cannot feed running downloads into a served UI; tests reach the coordinator directly (`crates/core/tests/download_lifecycle_tests.rs`). For a UI run with live transfers use a public URL, or a throwaway binary built with the guard argument flipped — never committed.
- **Login is rate-limited** (10 attempts per window, `crates/server/src/login.rs`). A browser script that logs in once per state or width locks itself out and then times out waiting for the list: log in once and reuse the cookies.
- **Progress lives in memory until a transfer ends.** Bytes and speed of a running download are not in SQLite; REST handlers overlay them with `Coordinator::overlay_live_progress`. A new endpoint that returns download rows needs the same call.
- **`DownloadEvent` is the WebSocket wire format.** It is serialized as is (fields next to `type`); `web-ui/src/lib/wsEvents.ts` parses exactly that. `crates/core/tests/event_wire_format.rs` and `web-ui/test/wsEvents.test.ts` pin the same payloads: change both sides together.

---

## Spec workflow profile

The project facts the shared spec skills (`spec`, `spec-build`, `spec-check`, `spec-extract`, `spec-fix`, `qa`) look up by these keys.

- **Design spec** — there is no single design spec: the architecture is this file (Architektur-Übersicht, Core-Features, Plugin-System, REST API, CLI) plus `docs/architecture.md`, `docs/plugin-api.md` (authoritative plugin API) and `docs/protocol-backends.md`. Feature specs live in `docs/specs/`; the older ones there (bold `**Status**` header instead of front matter) predate this workflow and are converted to the template when they are next revised.
- **Decisions** — a new numbered entry with its reason under "Wichtige Design-Entscheidungen" in this file; details in `docs/architecture.md`.
- **Code map** — `crates/core` (coordinator, chunk manager, bandwidth, queue, retry, post-processing, `protocol/` http / hls / dash / usenet, DLC container, storage, config, updater), `crates/extractors` (YouTube), `crates/plugin-runtime` (QuickJS engine, host API `krake.*`, IPC, the `krake-plugin-host` process, loader, registry, sandbox, SWC transpiler), `crates/server` (REST `api.rs`, `ws.rs`, Click'n'Load, NZBGet JSON-RPC, webhooks, update API), `crates/cli`, `web-ui/` (Svelte pages, components, `lib/api.ts`, stores), `packages/ui` (design tokens), `plugin-sdk/`, `plugins/` (+ `plugins/types/krake.d.ts`), `locales/`, `tauri/`. Integration tests per crate: `tests/README.md`.
- **Spec questions** — Which surfaces: core / protocol, plugin API (`krake.*`, `krake.d.ts`, `HOST_API_VERSION`), REST / WebSocket API, CLI, web UI, desktop, Docker? Which protocols (HTTP, Usenet, HLS, DASH)? Does persistent state change (SQLite schema, config TOML) and how is it migrated? Does the plugin sandbox or its permissions change? Strings for `locales/en.json` and `de.json`? Front matter: `surfaces: [core | plugin-api | rest | cli | web-ui | desktop | docker, …]`.
- **AC subject** — "Krake …" (or the surface: "The server …", "The CLI …").
- **Performance AC example** — "A queue of 1 000 downloads loads from SQLite in under 200 ms (integration test under `crates/core/tests/`, release build)".
- **Tests** — Rust unit tests inline, integration tests in `crates/*/tests/` (`tests/README.md`), plugin spec files `plugins/**/plugin.spec.ts` through `crates/plugin-runtime/tests/spec_runner.rs`, web UI unit tests (`pnpm --filter @amigo-labs/krake-web-ui test`), plugin SDK tests (`pnpm --filter @amigo-labs/krake-plugin-sdk test`). Red run: `cargo test -p <crate> <test_name>` (crates `krake-core`, `krake-extractors`, `krake-plugin-runtime`, `krake-server`, `krake-cli`), or the package's `test` script with the file. Tests use local test servers, never real hosters. Manual only for what cannot be automated (visual design, a real hoster or Usenet provider).
- **CI-only checks** — the release-build guard (pinned signing keys), `cargo audit`, `pnpm audit`, the Docker build and the release builds.
- **Build check** — `mkdir -p web-ui/dist && cargo check --workspace --locked` (needs `protoc`, like CI).
- **Conventions** — "Coding-Konventionen" above (English only, `thiserror` in libraries and `anyhow` only in binaries, async I/O without `block_on` in the core, TypeScript strict, Tailwind v4); lints are silenced only with `#[expect(.., reason = ..)]`, never `#[allow]` (workspace lint `allow_attributes`); UI follows the design system (`.impeccable.md`, summary in "Design System"); user-visible strings in `locales/en.json` and `de.json`; plugin code never gets direct network, filesystem or process access.
- **Stopping points** — a new dependency (`Cargo.toml`, `package.json`), a change of the plugin host API (`HOST_API_VERSION`, `krake.d.ts`, `docs/plugin-api.md`), a SQLite schema change, a widened plugin sandbox, new `unsafe`, a test that would have to be disabled, an architectural decision not yet under "Wichtige Design-Entscheidungen", an AC that turns out wrong.
- **Sizing** — one session builds about one M task (3–6 files, ~400 changed lines); a bigger feature is split into several specs.
- **Commits** — Conventional Commits (release-please: `feat` and `fix` start a release), one concept per commit, scopes `core`, `extractors`, `plugin-runtime`, `server`, `cli`, `web-ui`, `ui`, `sdk`, `plugins`, `tauri`, `docker`, `spec`. Example: `feat(core): resume DASH downloads per segment (AC-02, AC-03)`.
- **Improve** — for `/improve`. Serve the UI: `pnpm build:web-ui`, then `KRAKE_CONFIG_DIR=<scratch>/cfg KRAKE_SETUP_USER=admin KRAKE_SETUP_PASSWORD=<8+ chars> cargo run -p krake-server` from a scratch working directory (downloads land in `./downloads`), URL `http://127.0.0.1:1516/`; a builder's worktree is served with `pnpm exec vite dev --port <p> --strictPort` in its `web-ui/`, which proxies `/api` to that server. Playwright is not a dependency of this repository: `sweep.mjs --pkg <dir>` needs a folder whose `package.json` resolves the preinstalled `playwright`. Project checks: `node scripts/ui-lint.mjs` (hard bans, inline-style budget that only ratchets down; `--update` locks an improvement in), `pnpm --filter @amigo-labs/krake-ui check` (contrast pairs, primitives compile), `pnpm --filter @amigo-labs/krake-web-ui check`, `test` and `build`. Covering tests: `web-ui/test/*.test.ts` (grep for the target's helpers in `web-ui/src/lib/`); there is no browser suite. Stress setups live in the run's scratch folder: log in once and reuse the session cookie, set `localStorage` `locale` (`en` / `de`) and `theme`, seed downloads through `POST /api/v1/downloads` (see Gotchas for local test servers). Builder constraints: only the tokens of `packages/ui/src/tokens.css` (one accent, no raw colours), primitives from `packages/ui`, classes in `<style>` blocks rather than new `style=` attributes, every string through `tr()` with both languages in `web-ui/src/lib/i18n.ts`, keep the row classes `.dl-item`, `.dl-open`, `.dl-check`, `.dl-meta` and every accessible name, do not regress `docs/specs/ui-review-2026-07.md`.
- **Branches** — changes reach `main` through PRs; feature branch `feat/<spec-name>`.

<!-- amigo-skills:begin spec-workflow -->
<!-- Managed by .agents/sync.mjs from amigo-labs/skills (agents-md/). Edit it there, not here. -->

## Spec-driven workflow

Features are built spec-driven: first a feature spec `docs/specs/<name>.md` with testable acceptance criteria, then the implementation with tests, then an independent acceptance check. The project specifics (design spec, where decisions go, code map, tests, commits) are in the **spec workflow profile** of this file; the skills read it.

The skills live in `.agents/skills/` (read by Codex, Gemini CLI, Copilot, Cursor, OpenCode and others) and are mirrored to `.claude/skills/` for Claude Code. They are vendored from [amigo-labs/skills](https://github.com/amigo-labs/skills): change them there and run `node .agents/sync.mjs`, never edit the copies here. Skills only this repository has live in `.agents/skills/` too and are mirrored the same way.

| Skill | Purpose |
| ----- | ------- |
| `spec <name>` | Write or revise a feature spec with acceptance criteria and a clarification pass |
| `spec-build <name>` | Implement a feature spec test-first, one commit series per AC block, independent verifier loop |
| `spec-check <name>` | Independent acceptance against a full gate run; only it sets `status: implemented` |
| `spec-extract` | Spec from existing code, or current-state analysis of an open spec |
| `spec-fix <bug>` | Bug fix: root cause, regression test that is red first, minimal fix |
| `qa` | Full gate run (`.agents/gates.json`, mirrors CI) |

An agent that cannot invoke skills by name reads `.agents/skills/<name>/SKILL.md` and follows it. At the start of a task, an agent without a session-start hook runs `node .agents/skills/qa/scripts/open-specs.mjs` to see the open feature specs.

Two kinds of spec live in `docs/specs/`:

| Kind | Status | Owns |
| ---- | ------ | ---- |
| Design spec (named in the profile) | `status: draft`, no acceptance criteria | Architecture, data model, the decisions |
| Feature spec `docs/specs/<name>.md` | `status: todo` / `implemented`, `## Acceptance criteria` | One feature: what it does, how we prove it |

A feature spec references the design-spec sections it builds on and never contradicts them. A feature that needs an architectural decision (cross-crate or cross-package boundary, file format, new dependency, public API shape) gets a decision where the profile says, in the same commit as the spec. The spec index (profile **Spec index**, default `docs/specs/README.md`) lists the feature specs: order, dependencies, done, and the backlog of gaps not yet written as specs.

### Triage

Classify every new input before touching code:

| Input | Approach |
| ----- | -------- |
| New feature or changed behavior ("I want …", "it should …") | Start `spec`, no code before the spec. If the user says "just do it", implement directly. |
| Request touches an existing feature spec | Revise that spec in `spec`'s revision mode, never create a second one |
| Unclear whether small change or feature | Exactly one question: "Capture as spec" / "Implement directly" |
| Bug fix | `spec-fix`: directly, always with a regression test that is red before the fix. If the fix changes specified behavior, revise the spec too. |
| Refactoring, tooling, docs, questions | Directly, no spec |
| "Carry on", "build X", "implement the spec" | `spec-build <name>` |
| "Is X done?", acceptance after manual implementation | `spec-check <name>` |
| Document existing behavior | `spec-extract <area>` |

Only `spec-check` sets a spec to `status: implemented` — never by hand.

### Keeping knowledge

Project knowledge belongs in the repository, not on one machine:

- **Pitfalls and non-obvious knowledge** → `## Gotchas` in this file. Only what cannot be read from code or git history; no noise.
- **Architecture decisions** → where the profile's **Decisions** entry says.
- **Feature-local decisions** → `## Decisions` of the feature spec, with the reason.
- **Progress of a feature** → `## Evidence` of the feature spec (there is no separate status file).
- **Never** into an agent's private memory (e.g. Claude Code auto memory): it stays on one machine.

### Quality gates

- **Run the `qa` skill before finishing any coding task** and report the result. The gate commands are in `.agents/gates.json`: `stop` (seconds: formatting, clippy on the changed crates — the Claude Code Stop hook runs it after every turn that changed code), `fast` (before every commit — the Claude Code commit gate refuses a commit while one is red; other agents run `node .agents/skills/qa/scripts/gates.mjs fast` before committing) and `full` (mirrors CI, `node .agents/skills/qa/scripts/gates.mjs full`).
- **Fix the cause, never lower the gates.** No suppressions (the list is `suppressions` in `.agents/gates.json`), no weakened, skipped or ignored tests. The Claude Code guard hook refuses new suppressions; a justified one is added by the user.
- **Don't loosen the gate configuration**: `.agents/gates.json`, `.agents/skills/qa/scripts/`, `.claude/settings.json` and `.claude/hooks/` change only on the user's explicit request.
- Where a gate names a `fix` command (formatting, lint autofix), that fix is allowed; everything else is fixed in the code. Where a gate has a `note`, follow it.
- `STOP_GATE=off` / `COMMIT_GATE=off` in the environment switch the Claude Code Stop hook / commit gate off, e.g. for a bulk migration — only on the user's request.

<!-- amigo-skills:end spec-workflow -->
