# Plan: amigo-plugin-sdk

Ein Implementierungsplan für ein TypeScript-SDK, mit dem rund 90% aller JDownloader-artigen Plugins nachgebaut werden können, aufbauend auf der amigo-downloader Host-API (Tier 1 + 2).

## Scope & Non-Goals

**In Scope:**

- Pure-TypeScript-SDK, zusammen mit jedem Plugin gebundled (keine separate Runtime-Dependency).
- Abdeckung: Hoster-Plugins, Decrypter/Crawler-Plugins, Account-Login mit Session-Persistenz, Captcha-Abstraktion, JSON-APIs, HTML/Regex-Extraktion, Form-Handling, Link-Container (DLC/CCF/RSDF), Media-Manifeste (HLS/DASH).
- API-Design, das JDownloader-Plugins möglichst direkt übersetzbar macht, aber ohne Abkürzungen. Durchgängig `Browser`, `Page`, `Form`, `Context` usw. — keine `br`, `ctx`, `fmt`, `req`, `resp`.

**Out of Scope für dieses SDK:**

- Headless-Browser-Rendering (Tier 3, chromiumoxide) — eigenes späteres Feature.
- Segment-Download und Muxing von HLS/DASH — Sache der Rust-Download-Engine. Das SDK parst nur Manifeste.
- Plugin-Distribution/Registry (Marketplace) — eigenes Projekt.

## Voraussetzungen

Dieser Plan setzt voraus, dass Tier 1 + 2 der Host-API (http, html, json, crypto, util, js.eval) entweder fertig sind oder parallel entwickelt werden. Das SDK ist ein Layer darüber und kann in den frühen Phasen gegen einen Mock-Host entwickelt werden, bis die echte Host-API stabil ist.

## Grundregeln für den gesamten Plan

- Keine Abkürzungen in API-Namen. `Browser`, nicht `br`. `context`, nicht `ctx`. `response`, nicht `resp`. `format`, nicht `fmt`. `request`, nicht `req`.
- Alle öffentlichen APIs vollständig in TypeScript typisiert, keine `any` in öffentlicher API-Surface.
- Jede Phase ist eigenständig mergeable und hat Tests.
- Nach jeder Phase gibt es ein kurzes Beispiel-Plugin (oder Erweiterung eines bestehenden), das die neue Funktionalität demonstriert.

---
