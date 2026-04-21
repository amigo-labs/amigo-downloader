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

## Phase 0: Projekt-Setup & Workspace-Struktur

**Ziel:** Workspace aufsetzen, Build-Pipeline funktionsfähig, SDK-Skelett exportierbar.

**Aufgaben:**

- Monorepo-Entscheidung: SDK als Workspace-Member in amigo-downloader (bevorzugt, weil Host-API und SDK zusammen evolvieren).
- Package-Name: `@amigo/plugin-sdk`.
- Build-Pipeline mit SWC, Target ES2022, Output als ESM plus Type-Declarations.
- Strikte TypeScript-Config: `strict: true`, `noUncheckedIndexedAccess: true`, `exactOptionalPropertyTypes: true`.
- Linting/Formatting konsistent zum Rest des Projekts.
- Test-Framework: vitest.
- Ordnerstruktur:
  - `src/host/` — Host-API-Abstraktionsschicht
  - `src/browser/` — Browser, Page, CookieJar, Headers
  - `src/extraction/` — Regex-, JSON-, HTML-, Encoding-Helpers
  - `src/form/` — Form-Handling
  - `src/errors/` — Error-Typen und Factory-Funktionen
  - `src/captcha/` — Captcha-Abstraktionen
  - `src/plugin/` — Plugin-Definitionen (Hoster, Decrypter)
  - `src/context/` — PluginContext, AccountContext
  - `src/account/` — Session, Credentials
  - `src/media/` — HLS/DASH-Manifest-Parser
  - `src/container/` — DLC/CCF/RSDF-Parser
  - `src/javascript/` — js.eval-Wrapper (Tier 2)
  - `src/types/` — geteilte Typen (FileInfo, FormatInfo, DownloadLink, PluginManifest)
  - `test/` — Tests parallel zur src-Struktur
  - `test/fixtures/` — HTML-Snippets, JSON-Responses, Manifest-Beispiele, verschlüsselte Container-Samples

**Deliverable:** Leeres SDK-Package, das gebaut, gelintet und getestet werden kann. `pnpm test` läuft grün.

**Tests:** Smoke-Test, dass Package importierbar ist und Basis-Exports vorhanden sind.

---

## Phase 1: Host-API-Abstraktionsschicht

**Ziel:** Dünne, testbare Schicht zwischen SDK und Host. Ermöglicht Testing ohne echten rquickjs-Host.

**Aufgaben:**

- Interface `HostApi`, das alle Tier-1+2-Calls abstrahiert: http, html.parse, crypto, util, javascript.eval.
- Default-Implementation delegiert an globale Host-Functions, die vom rquickjs-Host injected werden.
- Mock-Implementation `MockHostApi` für Tests: konfigurierbar mit vordefinierten Responses, Request-Recording, Error-Injection, deterministischem Cookie-Jar.
- Dependency-Injection: `setHostApi(api)` und `getHostApi()`. In Production automatisch Default, in Tests Mock injiziert.
- Error-Konvertierung: strukturierte Host-Errors werden in SDK-interne Error-Typen gewrapped, damit Plugin-Autoren gegen eine stabile Error-API programmieren.

**Deliverable:** Alle Host-Calls gehen durch diese Schicht. Keine direkten `globalThis`-Zugriffe im restlichen SDK.

**Tests:**

- Mock-Host liefert konfigurierte Responses inklusive Headers, Status, Body.
- Error-Wrapping: Host-Timeout wird zu `PluginError` mit Code `"TimeoutError"`.
- Request-Recording: Tests können asserten, dass der richtige Call gemacht wurde.
- Abort: CancellationToken propagiert korrekt durch pending Calls.

---

## Phase 2: Browser, Page, CookieJar, Headers

**Ziel:** Das Herzstück. `Browser`-Objekt als Äquivalent zum JDownloader-`Browser`, aber mit ausgeschriebenen Namen.

**Aufgaben:**

**Browser-Klasse:**

- Methoden: `getPage`, `postPage`, `postPageRaw`, `headPage`, `request` (low-level Escape-Hatch).
- Mutable State nach jedem Request: `url`, `status`, `redirectLocation`, Response-Headers, Body.
- Konfiguration: `setFollowRedirects`, `setHeader`, `setHeaders`, `setUserAgent`, `setReferer`, `setTimeout`, `setMaxRedirects`.
- Getter: `getUrl()`, `getStatus()`, `getHeader(name)`.
- Body-Zugriffe: `body()` gibt Text, `bodyBytes()` gibt Uint8Array, `json<T>()` parst JSON.
- JDownloader-Style: `containsHTML(pattern: string | RegExp)` und `regex(pattern: string | RegExp): RegexResult`.
- `clone(): Browser` — neuer Browser mit eigenem State, aber geteilten Cookies und kopierten Default-Headers.
- `submitForm(form, overrides?)` — delegiert an Form-Phase.

**Page:**

- Immutable Snapshot des Browser-State nach einem Request.
- `browser.getPage(...)` mutiert den Browser und gibt gleichzeitig den Page-Snapshot zurück. Plugin-Autor kann wählen: "ich folge dem Browser-State" oder "ich halte den Snapshot fest".
- Methoden: `url`, `status`, `body()`, `find(selector)`, `findFirst(selector)`, `getForm(selector?)`, `getForms()`, `regex(pattern)`, `containsHTML(pattern)`, `json<T>()`.
- Keine Mutation-Methoden auf Page.

**CookieJar:**

- `get(url)`, `set(url, cookie)`, `clear()`, `clearHost(host)`.
- `export()` gibt serialisierbare Form (für Session-Persistenz).
- `import(cookies)` — umgekehrter Weg.
- Domain-Matching korrekt nach RFC 6265 (Subdomain-Wildcards).

**Headers:**

- Case-insensitive Map.
- Spezial-Setter: `setUserAgent`, `setReferer` (gehen auf spezifische Header, Referer kann explizit auf null gesetzt werden).

**Element-Abstraktion für DOM-Queries:**

- Interface `Element` mit `text()`, `html()`, `attr(name)`, `find(selector)`, `findFirst(selector)`, `parent()`.
- Null-safe: `attr()` gibt null statt Throw bei fehlendem Attribut.

**Deliverable:** Ein Browser, der gegen Mock-Host echte Request-Sequenzen durchspielen kann. Typische JDownloader-Patterns sind nachbaubar (außer Forms, die kommen in Phase 4).

**Tests:**

- Sequenz aus mehreren Requests mit Cookie-Persistenz.
- Referer wird automatisch vom vorherigen Response gesetzt, kann explizit überschrieben werden.
- Redirect-Handling: follow an/aus, `redirectLocation` korrekt gesetzt.
- `containsHTML` mit String und RegExp.
- `regex()` liefert korrekte Matches, Gruppen, Columns.
- Clone: zwei Browser-Instanzen divergieren unabhängig.
- Body über Host-Limit (z.B. 10 MB) wirft `BodyTooLargeError`.
- JSON-Parse auf Non-JSON-Body wirft strukturierten Error.

---

## Phase 3: Regex, JSON, HTML, Encoding

**Ziel:** Die kleinen Helpers, ohne die kein Plugin auskommt. JDownloader-Äquivalenz.

**Aufgaben:**

**Regex:**

- Standalone `regex(input, pattern): RegexResult` — für das JDownloader-Pattern `new Regex(str, "...").getMatch(0)` bei beliebigen Strings.
- `RegexResult`-Interface: `getMatch(groupIndex): string | null`, `getMatches(): string[][]`, `getColumn(index): string[]`, `matches(): boolean`.
- Leere Matches sauber: null/leeres Array, kein Throw.

**JSON:**

- `json.parse<T>(source): T`.
- `json.walk(object, path)` mit Path wie `"a/b/c"` — verschachtelter Zugriff, null wenn irgendwo unterwegs nicht vorhanden.
- Typ-sichere Getter: `json.getString`, `json.getNumber`, `json.getBool`, `json.getArray`, `json.getObject`.
- `json.extract(source, key)` — Regex-basierte Extraktion von JSON-Werten aus HTML/Text, Äquivalent zu JDownloaders `PluginJSonUtils.getJson`. Muss verschiedene Quote-Styles und Escape-Sequenzen handhaben.

**Encoding:**

- `encoding.urlEncode`, `encoding.urlDecode`.
- `encoding.htmlDecode` mit vollständiger Entity-Tabelle, `encoding.htmlEncode`.
- `encoding.unicodeDecode` — `\uXXXX` und `\xXX` Sequenzen in Strings auflösen.
- `encoding.base64Encode`, `encoding.base64Decode`, `encoding.hexEncode`, `encoding.hexDecode`.
- Alle Funktionen akzeptieren String oder Uint8Array wo sinnvoll.

**HTML-Helpers:**

- Parse-Funktion: `html.parse(htmlString, baseUrl?): Document` (intern via Host-API).
- Document-Interface mit den gleichen Methoden wie Page (find, findFirst, forms).
- Strip-HTML-Helper: `html.stripTags(source): string`.

**Deliverable:** Alle Extraction-Helpers funktionsfähig, gut getestet mit realen Fixtures aus JDownloader-Plugin-Szenarien.

**Tests:**

- Regex-Extraktion auf realen HTML-Snippets (Fixtures aus echten Hostern).
- HTML-Entity-Decoding: `&#x1F600;`, `&nbsp;`, numerische Entities, benannte Entities.
- JSON-Walk mit fehlenden Zwischenschritten gibt null.
- `json.extract` auf typischen Patterns: `"token":"abc"`, `'token': 'abc'`, `token: "abc"`.
- Base64-Roundtrip mit Binary-Daten (nicht nur ASCII).
- Unicode-Decode mit gemischten Escape-Styles.

---

## Phase 4: Form-Handling

**Ziel:** Das häufigste Szenario: Formular finden, Feld ändern, submitten.

**Aufgaben:**

- Klasse `Form` mit: `action`, `method`, `inputs: Record<string, string>`, `put(name, value)`, `get(name)`, `remove(name)`, `submit(overrides?): Promise<Page>`.
- Form wird aus HTML-Element extrahiert: action-URL resolved gegen Page-URL, Method extrahiert, alle `input`-, `select`- und `textarea`-Felder mit aktuellen Values gesammelt.
- `form.submit()` macht intern HTTP-Call über den zugehörigen Browser. Form behält Browser-Referenz.
- Multipart-Forms unterstützen, wenn Host-API das kann.
- CSS-Selektor-basierte Suche: `page.getForm("#login")` oder Index-basiert: `page.getForm(0)`. Ohne Argument: erstes Form-Element.
- Edge Cases: Forms ohne action (resolved auf aktuelle URL), Forms ohne method (Default GET), relative URLs in action (gegen Page-Base-URL resolven).

**Deliverable:** Form-Handling funktioniert für Login-Flows, Captcha-Submit-Flows, Multi-Step-Wizards.

**Tests:**

- Form-Extraktion aus HTML mit versteckten Feldern, Selects mit selected-Option, Checkboxes.
- Submit mit Overrides: vorhandenes Feld wird überschrieben, neues Feld wird hinzugefügt.
- Relative action-URL wird korrekt gegen Page-URL resolved.
- Form kann submitted werden, Browser-State wird danach aktualisiert.
- Form mit method="GET" appended Params an URL statt Body.
- Form ohne action submitted auf aktuelle URL.

---

## Phase 5: Error-System

**Ziel:** Typisierte, vom Host interpretierbare Errors mit Mapping auf JDownloader-LinkStatus-Semantik.

**Aufgaben:**

- Basisklasse `PluginError extends Error` mit `code: ErrorCode`, `retryAfterMilliseconds?: number`, `cause?: unknown`.
- Vollständiger `ErrorCode`-Union-Type: `"FileNotFound"`, `"PluginDefect"`, `"PremiumOnly"`, `"TemporarilyUnavailable"`, `"IpBlocked"`, `"CaptchaFailed"`, `"CaptchaUnsolvable"`, `"HosterUnavailable"`, `"DownloadLimitReached"`, `"AuthFailed"`, `"AuthRequired"`, `"Fatal"`, `"Retry"`, `"HttpError"`, `"TimeoutError"`, `"AbortError"`, `"ParseError"`, `"BudgetExceeded"`, `"PermissionDenied"`, `"BodyTooLarge"`, `"EvalError"`, `"ContainerDecryptionFailed"`, `"ManifestParseError"`.
- Factory-Funktionen im `errors`-Namespace, typisiert als `never`-returning. Das erlaubt `if (condition) errors.fileNotFound();` ohne explizites `throw`.
- Jede Factory akzeptiert optional Message und Zusatz-Infos (z.B. `retryAfterMilliseconds`).
- Serialisierung: Error muss über Host-Grenze hinweg serialisierbar sein. Der Rust-Host mappt `code` auf UI-Actions.
- Nicht-PluginError-Errors (echte Bugs, unbekannte Exceptions) werden vom Host in `PluginDefect` mit Stack-Trace-Attachment konvertiert.

**Deliverable:** Plugin-Code kann idiomatisch Errors werfen, die vom Host korrekt kategorisiert werden.

**Tests:**

- Jede Factory erzeugt Error mit korrektem Code.
- `retryAfterMilliseconds` wird korrekt serialisiert.
- Error-Cause-Chain funktioniert.
- Unbekannte Throws werden zu `PluginDefect` mit Original-Message und Stack.

---
