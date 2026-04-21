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

## Phase 6: Captcha-Abstraktion

**Ziel:** Plugin-Autor muss sich nicht um Solver-Backends kümmern. Host routet an konfigurierten Service oder UI-Prompt.

**Aufgaben:**

- Namespace `captcha` mit Funktionen:
  - `captcha.recaptchaV2(page, options?): Promise<string>`
  - `captcha.recaptchaV3(page, options): Promise<string>` (erfordert action)
  - `captcha.hcaptcha(page, options?): Promise<string>`
  - `captcha.turnstile(page, options?): Promise<string>`
  - `captcha.image(imageUrl, options?): Promise<string>` (mit Mode "text" oder "math")
  - `captcha.interactive(prompt, imageUrl?): Promise<string>` (User-Prompt)
  - `captcha.auto(page): Promise<CaptchaResult>` — Auto-Detection bekannter Widgets auf der Page
- Auto-Detection: scannt Page nach reCaptcha-iframe, hCaptcha-div, Turnstile-Marker, erkennt siteKey aus data-Attributen.
- Host-seitig wird der tatsächliche Solver konfiguriert (2Captcha, CapMonster, manuell via UI). SDK kennt nur das abstrakte Interface.
- Retry-Handling: `captcha.failed()` markiert den letzten Solve als inkorrekt für Service-Feedback.

**Deliverable:** Plugin-Autor löst Captchas mit einem Einzeiler. Kein Boilerplate.

**Tests:**

- Mock-Host liefert vordefinierten Token.
- Auto-Detection erkennt reCaptcha v2 aus Standard-HTML.
- Auto-Detection priorisiert spezifische Typen vor generischen.
- Failed-Feedback wird korrekt an Host propagiert.

---

## Phase 7: Plugin-Definitionen

**Ziel:** Top-Level-API, mit der ein Plugin geschrieben wird. Klare Trennung zwischen Hoster- und Decrypter-Plugins (wie JDownloader).

**Aufgaben:**

**Hoster-Plugin:**

- Funktion `definePlugin(config): Plugin` mit:
  - `id`, `version`, `match` (Array aus RegExp oder Glob-Pattern)
  - `checkAvailable(context): Promise<FileInfo>` — optionaler schneller Existence/Size-Check
  - `extract(context): Promise<FormatInfo[]>` — der Haupt-Extraktions-Flow
  - `account?: AccountConfig` — optional für Premium-Hoster
- `PluginContext` enthält: `url`, `browser: Browser`, `account?: AccountContext`, `config: PluginConfig`, `log`, `wait`, `link(url): DownloadLink` (createDownloadlink-Äquivalent), `format(info): FormatInfo`, `formats(infos): FormatInfo[]`, `abortSignal`.

**Decrypter/Crawler-Plugin:**

- Funktion `defineDecrypter(config): Plugin` mit:
  - `id`, `version`, `match`
  - `decrypt(context): Promise<string[] | DownloadLink[]>` — gibt URLs oder vorbereitete DownloadLinks zurück

**Shared Types:**

- `FileInfo`: filename, size, hash, availability, mimeType.
- `FormatInfo`: url, filename, size, headers, quality, codec, mediaType (für HLS/DASH siehe Phase 9).
- `DownloadLink`: url, filename, referer, properties (Key-Value-Bag für plugin-spezifische Metadaten).
- `PluginConfig`: typisierter Zugriff auf plugin-spezifische User-Settings (im Manifest deklariert).

**Manifest-Integration:**

- Plugin-Manifest (TOML) deklariert id, version, permissions, host_patterns, config-Schema, sdk_version.
- SDK validiert zur Load-Time gegen Manifest.

**Deliverable:** Ein vollständiges Hoster-Plugin und ein Decrypter-Plugin lassen sich als Ende-zu-Ende-Beispiel bauen und gegen Mock-Host ausführen.

**Tests:**

- Plugin-Definition wird korrekt exportiert.
- URL-Matching gegen Regex und Glob.
- Context-Injection funktioniert.
- Result-Type-Konvertierung (string[] vs DownloadLink[]) ist korrekt.

---

## Phase 8: Account- und Session-System

**Ziel:** Login-Flows, Session-Persistenz, Premium-Account-Handling — JDownloader-kompatibel.

**Aufgaben:**

- `AccountConfig` im Hoster-Plugin mit:
  - `login(context, credentials): Promise<Session>` — führt Login durch, gibt Session zurück.
  - `check(context, session): Promise<AccountStatus>` — validiert Session, gibt Status inklusive Ablaufdatum und Premium-Flag zurück.
  - `logout?(context, session): Promise<void>` — optional.
- `Session`-Struktur: `cookies`, `headers` (für Auth-Header wie Bearer-Tokens), `metadata` (Key-Value für plugin-spezifisches wie Refresh-Token, UserId).
- `AccountContext` im Plugin-Context: `session: Session`, `status: AccountStatus`, `credentials` (nur während Login verfügbar).
- Host-seitige Persistenz: SDK ruft `account.persist(session)` auf, Host speichert in SQLite. Automatisches Rehydrate beim nächsten Plugin-Call.
- Mehrfach-Accounts: Host kann mehrere Accounts pro Hoster haben, SDK bekommt den aktuell ausgewählten im Context.
- Rate-Limiting pro Account: Host trackt Requests pro Account, SDK kann `context.account.canRequest()` prüfen.

**Deliverable:** Hoster-Plugin mit Premium-Login, das Session zwischen Runs persistiert.

**Tests:**

- Login mit gültigen Credentials gibt Session zurück.
- Login mit falschen Credentials wirft `AuthFailed`.
- Check mit abgelaufener Session gibt `valid: false`.
- Session-Export/Import-Roundtrip ist verlustfrei.
- Mehrere Accounts pro Hoster funktionieren parallel ohne State-Kollision.

---

## Phase 9: Media-Manifest-Support (HLS und DASH)

**Ziel:** Plugin kann HLS-Master-Playlists und DASH-Manifeste parsen, Varianten enumerieren, Qualität selektieren und eine fertige FormatInfo an die Download-Engine übergeben. Das SDK parst nur — der tatsächliche Segment-Download und das Muxing sind Sache der Rust-Engine.

**Aufgaben:**

**HLS (M3U8):**

- `media.hls.parseMaster(content, baseUrl?): HlsMasterPlaylist` — parst Master-Playlist.
- `HlsMasterPlaylist` enthält: `variants: HlsVariant[]`, `audioTracks: HlsAudioTrack[]`, `subtitleTracks: HlsSubtitleTrack[]`.
- `HlsVariant` mit: `url`, `bandwidth`, `resolution: { width, height }?`, `codecs: string[]`, `frameRate?`, `audioGroup?`, `subtitleGroup?`.
- `media.hls.parseMedia(content, baseUrl?): HlsMediaPlaylist` — parst Variant-Playlist (Liste von Segmenten). Meist nicht nötig im Plugin, aber für Debugging und Spezialfälle verfügbar.
- Alle URLs in Parse-Ergebnissen werden gegen `baseUrl` absolut gemacht.
- Unterstützt: `#EXT-X-STREAM-INF`, `#EXT-X-MEDIA`, `#EXT-X-I-FRAME-STREAM-INF`, `#EXT-X-VERSION`, `#EXT-X-INDEPENDENT-SEGMENTS`.
- Encrypted Streams: parsiert `#EXT-X-KEY`-Tag, aber Decryption ist Engine-Sache.

**DASH (MPD):**

- `media.dash.parse(content, baseUrl?): DashManifest` — parst MPD-XML.
- `DashManifest` mit: `periods: DashPeriod[]`, `type: "static" | "dynamic"`, `duration?`.
- `DashPeriod` mit: `adaptationSets: DashAdaptationSet[]`.
- `DashAdaptationSet` mit: `mimeType`, `contentType: "video" | "audio" | "text"`, `representations: DashRepresentation[]`.
- `DashRepresentation` mit: `id`, `bandwidth`, `width?`, `height?`, `codecs?`, `baseUrl?`, `segmentTemplate?`.
- Unterstützt: SegmentTemplate, SegmentList, SegmentBase. BaseURL-Resolution über verschachtelte Elemente.

**Selection-Helpers:**

- `media.selectBestVariant(variants, criteria?): Variant` — default: höchste Bandwidth. Criteria optional: `maxHeight`, `preferCodec`, `maxBandwidth`.
- `media.selectWorstVariant(variants, criteria?): Variant`.
- `media.filterByResolution(variants, { min?, max? }): Variant[]`.
- `media.filterByCodec(variants, codecPattern: RegExp): Variant[]`.
- Helpers funktionieren sowohl für HLS-Variants als auch DASH-Representations über ein gemeinsames Mini-Interface.

**FormatInfo-Integration:**

- `FormatInfo.mediaType` kann sein: `"direct"`, `"hls"`, `"dash"`.
- Für HLS/DASH enthält FormatInfo die Manifest-URL und optional eine vorselektierte Variant-URL.
- Plugin-Autor kann entweder das komplette Master-Manifest zurückgeben (Engine/User wählt) oder eine konkrete Variant (Plugin wählt).

**Deliverable:** Plugin-Autor kann ein Vimeo-artiges Plugin schreiben, das ein Master-M3U8 aus einer API holt, die Varianten parst, die beste auswählt und als FormatInfo zurückgibt.

**Tests:**

- Parse-Tests mit echten Master-Playlists von Vimeo, YouTube-kompatiblen Quellen, Brightcove, generischen HLS-Samples.
- DASH-Parse mit Samples von typischen Anbietern (akamai-style, bbc-style, youtube-style).
- BaseURL-Resolution: relative Segment-URLs gegen Master-URL.
- Selection-Helpers: korrekte Variante wird nach Bandwidth/Resolution-Criteria ausgewählt.
- Encrypted Stream: `#EXT-X-KEY` wird erkannt und in Metadaten aufgenommen, Parsing schlägt nicht fehl.
- Malformed Manifest wirft `ManifestParseError` mit Kontext.

---

## Phase 10: Container-Format-Support (DLC, CCF, RSDF)

**Ziel:** Built-in Decrypter für die drei klassischen Link-Container-Formate. SDK stellt Parser bereit, und Amigo shipped mit built-in Decrypter-Plugins, die diese Parser nutzen.

**Wichtige Design-Entscheidung:** DLC erfordert historisch einen externen Key-Exchange-Service (original von jdownloader.org betrieben, proprietär). Der Plan unterstützt DLC nur in Verbindung mit einem konfigurierbaren Service-Endpoint. RSDF und CCF haben öffentlich bekannte feste Schlüssel und funktionieren offline.

**Aufgaben:**

**RSDF (einfachster Fall, öffentlicher fester Schlüssel):**

- `container.rsdf.parse(content: Uint8Array | string): string[]` — liefert Liste der entschlüsselten URLs.
- Implementation: Base64-dekodieren, mit bekanntem AES-128-CBC-Schlüssel und bekanntem IV entschlüsseln, zeilenweise splitten.
- Feste Konstanten als interne Module-Level-Werte.

**CCF (öffentlicher fester Schlüssel, XML-Format):**

- `container.ccf.parse(content: Uint8Array): CcfContainer` mit `packageName?`, `links: CcfLink[]`.
- `CcfLink` mit: `url`, `filename?`, `size?`, `password?`.
- AES-128-entschlüsselt, dann XML-Parse.

**DLC (erfordert Service):**

- `container.dlc.parse(content: string | Uint8Array, options: DlcOptions): Promise<DlcContainer>`.
- `DlcOptions` erfordert `keyExchangeEndpoint: string` — URL eines DLC-Key-Service. Host-Config stellt Default-Endpoint bereit (konfigurierbar durch User).
- Flow: letzte 88 Zeichen als Service-Key extrahieren, an Endpoint POSTen, AES-Key zurückbekommen, Rest des Inhalts entschlüsseln (AES-CBC), als XML parsen.
- `DlcContainer` mit: `packageName?`, `uploadDate?`, `maxMirrors?`, `links: DlcLink[]`.
- Fehler-Fälle: Service nicht erreichbar, Service lehnt Key ab, Entschlüsselung schlägt fehl, XML malformed — alle sauber mit `ContainerDecryptionFailed` gehandhabt.
- Dokumentation macht klar, dass DLC-Support vom gewählten Service abhängt und User gegebenenfalls einen eigenen Endpoint konfigurieren muss.

**Auto-Detection:**

- `container.detect(content: Uint8Array): "dlc" | "ccf" | "rsdf" | null` — Heuristik basierend auf Dateiendung, Magic-Bytes und Content-Struktur.
- Built-in Decrypter-Plugin `amigo-container` matched `.dlc`, `.ccf`, `.rsdf`-URLs und Dateipfade, dispatched intern an die richtige Parser-Funktion.

**File-Input-Handling:**

- Container-Dateien kommen üblicherweise als lokale Datei, nicht als URL. Host-API muss `util.readFile(path)` exposen (eingeschränkt auf vom User ausgewählte Files).
- Alternativ können Container als Base64-inline-Content übergeben werden.

**Deliverable:** Built-in Decrypter-Plugin `amigo-container`, das alle drei Formate unterstützt, plus dokumentierte SDK-Helper für Custom-Container-Plugins.

**Tests:**

- RSDF-Parse mit echten Sample-Dateien aus JDownloader-Test-Suite (oder reproduzierbaren Testvektoren).
- CCF-Parse mit Sample-Dateien.
- DLC-Parse gegen Mock-Service, der vordefinierte Keys liefert.
- Malformed Container wirft `ContainerDecryptionFailed` mit verständlicher Message.
- Auto-Detection über Magic-Bytes und Content.
- End-to-End: Built-in-Plugin akzeptiert URL zu `.rsdf`, liefert URL-Liste.

---

## Phase 11: JavaScript-Eval-Wrapper (Tier 2)

**Ziel:** Dünner, sicherer Wrapper um die Host-js.eval-Funktion. Für obfuscated JS-Snippets, die Download-Links client-seitig berechnen.

**Aufgaben:**

- Namespace `javascript` mit:
  - `javascript.run<T>(code, input?, options?): Promise<T>` — evaluiert Code in separatem Sub-Context. `input` wird als globale Variable gesetzt.
  - `javascript.unpackDeanEdwards(code): string` — unpackt Dean-Edwards-Packer-Output (häufigster Case bei Filehostern). Ruft intern `javascript.run` auf und extrahiert den unpacked Source.
  - `javascript.unpackEval(code): string` — generische Unwrap-Logik für `eval(...)`-wrapped Code.
- Jeder Call bekommt eigene Memory- und CPU-Limits, die über Host-API konfiguriert werden können. Default: 16 MB, 5 Sekunden.
- Timeout und Memory-Overflow geben `EvalError` zurück.
- Plugin-Permission `javascript_eval` muss im Manifest deklariert sein, sonst wirft der Wrapper `PermissionDenied`.

**Deliverable:** Plugin-Autor kann obfuscated JS aus einer Page extrahieren, unpacken und das Ergebnis als Link weiterverwenden.

**Tests:**

- Einfaches `javascript.run("return 1+1")` liefert 2.
- Dean-Edwards-Unpack mit realem Packer-Output aus Filehoster-Sample.
- Timeout wird getriggert bei Endlosschleife.
- Memory-Limit wird bei Overflow getriggert.
- Ohne Permission: `PermissionDenied`.

---

## Phase 12: Utility-Helpers

**Ziel:** Die kleinen Helfer, die Plugin-Autoren ständig brauchen und sonst selbst implementieren müssten.

**Aufgaben:**

- `parseSize(input: string): number` — versteht `"1.5 GB"`, `"500 MiB"`, `"1,024 KB"`, gibt Bytes.
- `parseDuration(input: string): number` — `"2h 30m"`, `"1:30:00"`, gibt Millisekunden.
- `parseDate(input: string, locale?: string): Date` — akzeptiert relative Zeitangaben (`"2 hours ago"`, `"yesterday"`) und absolute Daten in verschiedenen Locales.
- `formatFilename(unsafe: string, options?): string` — sanitized für Dateisystem (entfernt/ersetzt ungültige Zeichen), kürzt auf maximale Länge, optional Diakritika-Normalisierung.
- `Context.wait(milliseconds)` — respektiert AbortSignal, im Context bereits verfügbar.
- `Context.log(level, message, metadata?)` — strukturiertes Logging in die Plugin-Log-Kapsel.
- `Context.progress(current, total, message?)` — für lange Operationen.

**Deliverable:** Plugin-Autoren müssen weniger eigenen Code schreiben.

**Tests:**

- Parse-Funktionen mit einer Sammlung realer Input-Strings aus verschiedenen Hostern.
- Filename-Sanitization mit Unicode, reservierten Windows-Namen, Überlangem Input.
- `wait` bricht bei Abort korrekt ab.

---

## Phase 13: Entwickler-Experience und CLI

**Ziel:** Plugin-Entwicklung soll sich gut anfühlen. Local-Testing ohne die ganze amigo-downloader-App starten zu müssen.

**Aufgaben:**

- CLI-Befehl `amigo plugin new <id>` — scaffolded Plugin-Projekt mit Template, Manifest, passendem `tsconfig.json`, Test-Setup.
- `amigo plugin test <url>` — spawnt einen minimalen Runner mit Host-API-Mock (oder Real-Host mit Sandbox), lädt das Plugin, ruft extract/decrypt mit der URL auf, printed strukturiertes Result plus Log-Output.
- `amigo plugin build` — SWC-Transpilation, Manifest-Validation, Bundle in `.amigoplugin`-Tarball.
- `amigo plugin install <path>` — lädt lokal in laufende amigo-downloader-Instanz.
- `amigo plugin validate <path>` — statische Checks: Manifest-Schema, SDK-Version-Compatibility, Permission-Consistency.
- Watch-Mode für `test`: Hot-Reload bei Code-Changes.

**Deliverable:** Plugin-Autor kann in unter 2 Minuten von `plugin new` zu funktionierendem Plugin gegen eine echte URL kommen.

**Tests:**

- Scaffolding generiert Plugin, das direkt buildbar ist.
- Test-CLI liefert strukturiertes Result oder verständlichen Error.
- Build erzeugt valides Tarball.
- Watch-Mode detected Änderungen korrekt.

---

## Phase 14: Integrations-Test-Suite

**Ziel:** Ende-zu-Ende-Tests, die echte Plugin-Szenarien gegen Mock-Server durchspielen. Verhindert Regressions bei API-Änderungen.

**Aufgaben:**

- Test-Fixture-Server (lokal, mit vitest): simuliert typische Filehoster-Flows (Landing-Page, Countdown, Captcha, Final-Link), API-basierte Crawler (JSON-Endpoints mit Pagination), HLS-Streaming-Seite, RSDF-Container-Download.
- Für jedes relevante JDownloader-Pattern eine Integration-Test-Suite:
  - Simpler Filehoster mit Countdown und Captcha.
  - API-Crawler mit Pagination und Account-Auth.
  - Folder-Decrypter mit mehreren Levels.
  - Streaming-Seite mit HLS-Master und Variant-Selection.
  - RSDF/CCF/DLC-Container-Flows.
  - Redirect-Shortener (Ouo.io-Style).
- Tests laufen gegen den Mock-Server, validieren erwartetes Plugin-Verhalten Ende-zu-Ende.
- Performance-Budget: jeder Test unter 5 Sekunden.

**Deliverable:** Robuste Test-Suite, die jedes Mal läuft, wenn SDK-Code geändert wird. Garantiert, dass Änderungen keine bestehenden Plugin-Patterns brechen.

**Tests:** Die Suite ist der Test. Zusätzliche Meta-Tests validieren, dass der Mock-Server korrekt funktioniert.

---

## Phase 15: Dokumentation und Beispiel-Plugins

**Ziel:** Plugin-Autoren können ohne Code-Reading des SDK-Source sofort loslegen.

**Aufgaben:**

- API-Referenz aus JSDoc/TSDoc generiert.
- Tutorial: "Dein erstes Plugin in 10 Minuten" — Schritt-für-Schritt für einen einfachen Hoster.
- Cookbook: 15+ Standard-Rezepte:
  - Hoster mit Countdown und reCaptcha
  - API-basierter Crawler mit Pagination
  - Folder-Decrypter
  - Premium-Account-Login und Session-Persistenz
  - HLS-Streaming-Extraktion
  - RSDF-Container-Import
  - Obfuscated-JS-Link-Derivation
  - Shortener-Resolver
  - URL-Regex-Match mit mehreren Varianten
  - Multipart-Form-Submit (z.B. Datei-Upload-Seiten)
  - Session-Rehydrate mit abgelaufenem Token (Refresh-Flow)
  - Redirect-Chain-Follow
  - Rate-Limited API mit Backoff
  - Response-Validierung und typsichere Parsing
  - Debug-Logging-Patterns
- Mapping-Tabelle: JDownloader-API zu amigo-plugin-sdk-API, als Referenz für Porting.
- Migrationsleitfaden für Leute, die JDownloader-Plugins portieren wollen.

**Deliverable:** Ein Plugin-Autor mit JDownloader-Erfahrung kann ein bestehendes Plugin in unter einer Stunde portieren.

**Tests:**

- Jedes Cookbook-Beispiel hat einen Integration-Test in Phase 14.
- Dokumentation wird auf Build-Time gegen die echte API geprüft (kein Drift).

---
