# Plan: YouTube + HLS/DASH Streaming Support

## Context

YouTube-Downloads funktionieren nicht, weil der aktuelle `ANDROID` innertube-Client keine direkten URLs mehr liefert. Außerdem fehlen HLS- und DASH-Downloader komplett — ohne die kann amigo-dl keine Streaming-Formate laden. Die YouTube-Logik liegt aktuell falsch im Core-Crate statt in einem eigenen Extractor.

**Ziel:** YouTube-Downloads zum Laufen bringen, HLS/DASH als Protokolle unterstützen, YouTube aus Core rausnehmen, `--verbose` Flag fürs Debugging.

---

## Architektur-Entscheidungen

### YouTube → neues `crates/extractors/` Crate
- YouTube ist site-spezifisch, nicht Core-Protokoll
- Extractors transformieren Page-URLs in downloadbare Stream-URLs
- Server-Crate kann es später auch nutzen (kein Duplizieren)
- Pattern für weitere Sites (Twitch, Vimeo) erweiterbar

### HLS/DASH → `crates/core/src/protocol/`
- Sind Transport-Protokolle wie HTTP/NNTP → gehören zu Core
- Jeder Extractor/Plugin der eine m3u8/mpd URL liefert kann sie nutzen

### N-Parameter → `rquickjs` (embedded QuickJS)
- Ohne N-Parameter-Lösung: YouTube throttled auf ~50KB/s
- `rquickjs` ist ein embedded JS-Runtime (~2MB Binärgröße)
- Kompiliert QuickJS aus C-Source, läuft auf Windows mit MSVC
- JS-Ausführung ist schnell (<50ms), via `spawn_blocking`

### YouTube-Client → `android_vr` (Oculus Quest 3)
- Liefert direkte URLs ohne PO-Tokens oder Signature-Decryption
- clientName: `ANDROID_VR`, clientVersion: `1.65.10`
- deviceMake: Oculus, deviceModel: Quest 3, androidSdkVersion: 32
- userAgent: `com.google.android.apps.youtube.vr.oculus/1.65.10 (Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A1) gzip`
- Limitierung: "Made for kids" Videos gehen nicht → Fallback auf `web_embedded`

---

## Umsetzung in 8 Phasen

### Phase 0: Branch + Scaffolding
1. Branch `feat/youtube-hls-dash` erstellen
2. `crates/extractors/` mit `Cargo.toml` + `src/lib.rs` anlegen
3. Workspace-Root: `amigo-extractors` registrieren
4. CLI: `amigo-extractors` als Dependency
5. `cargo check` verifizieren

### Phase 1: YouTube aus Core rausnehmen
6. `crates/core/src/protocol/youtube.rs` → `crates/extractors/src/youtube/` verschieben + aufteilen:
   - `url_parser.rs` — Video-ID Extraktion
   - `innertube.rs` — API-Client
   - `formats.rs` — Format-Parsing, Qualitäts-Auswahl
   - `mod.rs` — Public API
7. `Extractor` Trait + `ExtractedMedia` Types in `crates/extractors/src/`
8. CLI updaten: `amigo_extractors` statt `amigo_core::protocol::youtube`
9. `pub mod youtube;` aus `crates/core/src/protocol/mod.rs` entfernen
10. Tests migrieren, `cargo test` verifizieren

### Phase 2: YouTube-Extraktion fixen (`android_vr`)
11. `innertube.rs` umschreiben auf `android_vr` Client-Config
12. Korrekter User-Agent + Headers (`X-YouTube-Client-Name: 28`)
13. Fallback: `web_embedded` (clientName `WEB_EMBEDDED_PLAYER`, clientVersion `1.20260115.01.00`, embedUrl `https://www.reddit.com/`)
14. Watch-Page Fallback beibehalten
15. Testen mit `--verbose`

### Phase 3: `--verbose` Flag
16. `-v/--verbose` Flag zum `Cli` Struct
17. `tracing_subscriber` Init am Anfang von `main()`
18. Default: `warn`, verbose: `debug`, `RUST_LOG` env hat Vorrang

### Phase 4: N-Parameter Challenge
19. `rquickjs` zu Workspace + Extractors Dependencies
20. `n_challenge.rs`: Player-JS fetchen, N-Funktion extrahieren, via QuickJS ausführen
21. In-Memory Cache für Player-JS (nach URL gekeys)
22. N-Parameter in allen Stream-URLs transformieren
23. Testen: Download-Speed >1 MB/s verifizieren

### Phase 5: HLS Downloader
24. `m3u8-rs` zu Workspace + Core Dependencies
25. `crates/core/src/protocol/hls.rs` erstellen:
    - Master-Playlist parsen → beste Variante wählen
    - Media-Playlist parsen → Segment-URLs sammeln
    - Parallele Segment-Downloads (8 concurrent)
    - Segments zusammenfügen (Concatenation)
    - Progress-Reporting via `watch::Sender<DownloadProgress>`
26. `Protocol::Hls` zum Enum, `pub mod hls;` registrieren
27. CLI: HLS-Dispatch in `direct_download()`

### Phase 6: DASH Downloader
28. `dash-mpd` zu Workspace + Core Dependencies
29. `crates/core/src/protocol/dash.rs` erstellen:
    - MPD parsen, beste Representation wählen
    - Segment-URLs generieren (SegmentTemplate mit `$Number$`)
    - Parallele Segment-Downloads
    - Reassembly (Init-Segment + Media-Segments)
30. `Protocol::Dash` zum Enum, `pub mod dash;` registrieren
31. CLI: DASH-Dispatch in `direct_download()`

### Phase 7: Integration
32. Coordinator updaten für HLS/DASH in `start_download()`
33. `detect_protocol()` für `.m3u8` und `.mpd` URLs erweitern
34. `ResolvedUrl` Struct um `protocol` Feld erweitern

### Phase 8: Tests + Polish
35. Unit-Tests: HLS-Manifest-Parsing, DASH-Manifest-Parsing, YouTube URL-Parsing
36. Integration-Test: YouTube Full-Flow (`#[ignore]`, braucht Netzwerk)
37. `cargo clippy`, `cargo fmt`

---

## Dateien

### Neu erstellen
```
crates/extractors/Cargo.toml
crates/extractors/src/lib.rs
crates/extractors/src/error.rs
crates/extractors/src/traits.rs
crates/extractors/src/youtube/mod.rs
crates/extractors/src/youtube/innertube.rs
crates/extractors/src/youtube/url_parser.rs
crates/extractors/src/youtube/formats.rs
crates/extractors/src/youtube/n_challenge.rs
crates/core/src/protocol/hls.rs
crates/core/src/protocol/dash.rs
```

### Modifizieren
```
Cargo.toml                              — workspace members + deps (rquickjs, m3u8-rs, dash-mpd)
crates/core/Cargo.toml                  — m3u8-rs, dash-mpd
crates/core/src/protocol/mod.rs         — youtube entfernen, hls/dash hinzu, Protocol enum
crates/core/src/coordinator.rs          — HLS/DASH dispatch
crates/cli/Cargo.toml                   — amigo-extractors dep
crates/cli/src/main.rs                  — verbose flag, extractor integration, protocol dispatch
```

### Löschen
```
crates/core/src/protocol/youtube.rs     — verschoben nach extractors
```

---

## Neue Dependencies

| Crate | Version | Zweck |
|-------|---------|-------|
| `rquickjs` | 0.9 | JS-Runtime für N-Parameter Challenge |
| `m3u8-rs` | 6 | HLS Manifest Parsing |
| `dash-mpd` | 0.17 | DASH/MPD Manifest Parsing |

---

## Key Types

```rust
// crates/extractors/src/lib.rs
pub struct ExtractedMedia {
    pub title: String,
    pub streams: Vec<MediaStream>,
}

pub struct MediaStream {
    pub url: String,
    pub protocol: StreamProtocol,
    pub quality_label: String,
    pub height: u32,
    pub mime_type: String,
    pub filesize: Option<u64>,
    pub has_audio: bool,
    pub has_video: bool,
}

pub enum StreamProtocol { Http, Hls, Dash }
```

---

## Verifizierung

1. `cargo test --workspace` — alle Tests grün
2. `cargo clippy --workspace` — keine Warnings
3. `amigo-dl --verbose https://www.youtube.com/watch?v=1mzl2Oo8Ncw` — zeigt Debug-Output, lädt Video runter
4. `amigo-dl https://example.com/file.zip` — normaler HTTP-Download funktioniert weiterhin
5. Download-Speed >1 MB/s (N-Parameter korrekt gelöst)

---

## Risiken

- **`rquickjs` Build auf Windows**: Braucht MSVC C-Compiler (normalerweise mit Rust-Toolchain vorhanden). Früh in Phase 4 testen.
- **YouTube API-Änderungen**: Client-Config als Daten strukturieren, nicht in Logik hardcoden → einfach austauschbar.
- **Audio/Video Muxing**: `android_vr` liefert Combined-Formats bis 720p. Für höhere Qualitäten braucht es separate Streams + Muxing (späteres Feature).

---

## yt-dlp Referenz

Basierend auf Analyse von `yt-dlp/yt_dlp/extractor/youtube/` und `yt-dlp/yt_dlp/downloader/`:

### Innertube Client-Priorität (yt-dlp)
1. `android_vr` — kein PO-Token, kein Signature-Decryption, kein JS-Player nötig
2. `web_embedded` — kein PO-Token, braucht aber JS-Player für Signature
3. `web_safari` — PO-Token nötig, liefert HLS-Formate
4. `tv` — kein PO-Token, braucht JS-Player

### Downloader in yt-dlp
- **HttpFD** — Standard HTTP mit Resume
- **HlsFD** — m3u8 Manifest, AES-128 Decryption, Segment-Downloads
- **DashSegmentsFD** — MPD Manifest, Fragment-Downloads, Resume
- **FFmpegFD** — Fallback über FFmpeg
- Plus: RTMP, F4M, ISM, RTSP, WebSocket, etc. (für YouTube irrelevant)
