# Plugin API Reference

Plugins are TypeScript (`.ts`) files that run in a sandboxed QuickJS VM. TypeScript is transpiled to JavaScript at load time via SWC.

## Quick Start

1. Copy `plugins/template/plugin.ts`
2. Implement the required exports
3. Drop into `plugins/hosters/<your-plugin>/` or `plugins/extractors/<your-plugin>/`
4. It auto-loads on startup (hot-reload supported)

Type definitions for IDE support: `plugins/types/amigo.d.ts`

## Plugin Interface

```typescript
interface AmigoPlugin {
    // ── Required ──
    id: string;                  // Unique ID, e.g. "mega-nz"
    name: string;                // Display name, e.g. "MEGA.nz"
    version: string;             // Semver, e.g. "1.0.0"
    apiVersion: number;          // Host-API major, currently 1 (see below)
    urlPattern: string;          // Regex matching URLs this plugin handles
    resolve(url: string): DownloadPackage;

    // ── Optional ──
    description?: string;
    author?: string;
    permissions?: {
        domains?: string[];      // Hosts amigo.http* may reach (see below)
    };
    checkOnline?(url: string): "online" | "offline" | "unknown";
    login?(username: string, password: string): boolean;
    supportsPremium?(): boolean;
    decryptContainer?(data: string): string[];
    resolveFolder?(url: string): string[];
    postProcess?(context: PostProcessContext): PostProcessResult;
}
```

### Minimal Example

```typescript
/// <reference path="../types/amigo.d.ts" />

module.exports = {
    id: "example",
    name: "Example Hoster",
    version: "1.0.0",
    apiVersion: 1,
    permissions: { domains: ["example.com"] },
    urlPattern: "https?://example\\.com/.+",

    resolve(url: string): DownloadPackage {
        const head = amigo.httpHead(url);
        return {
            name: amigo.urlFilename(url) || "Download",
            downloads: [{
                url,
                filename: amigo.urlFilename(url),
                filesize: head.headers["content-length"]
                    ? parseInt(head.headers["content-length"]) : null,
                chunks_supported: head.headers["accept-ranges"] === "bytes",
                max_chunks: null,
                headers: null,
                cookies: null,
                wait_seconds: null,
                mirrors: [],
            }],
        };
    },
} satisfies AmigoPlugin;
```

### Host-API version (`apiVersion`)

`apiVersion` is the major version of the `amigo.*` host API the plugin is
written against. The runtime's single source of truth is `HOST_API_VERSION`
in `crates/plugin-runtime/src/lib.rs` (currently **1**); the SDK mirrors it as
`plugin.HOST_API_VERSION`.

Compatibility policy:

- **Within a major** the surface only grows: new functions, new optional
  arguments, new fields on returned objects. A plugin written for `1` keeps
  working on every host that implements `1`.
- **A new major** is required for anything that can break an existing plugin:
  renaming or removing a function, changing a return shape, tightening an
  argument.
- **The previous major stays supported for one release** after a bump
  (`MIN_SUPPORTED_API_VERSION`), so plugins can migrate.
- A plugin declaring a major the host does not support is **refused at load**
  with an error naming both versions. The marketplace hides such plugins and
  update checks skip them, so the host never offers an install it cannot load.
- Transition: a plugin without `apiVersion` is loaded as `1` with a warning.
  This fallback will be removed — declare it.

### Network permissions (`permissions.domains`)

A plugin declares the hosts it may reach through `amigo.http*`:

```typescript
permissions: { domains: ["api.real-debrid.com", "*.rdeb.io"] },
```

- Entries are hosts, not URLs: an exact host (`api.real-debrid.com`) or one
  leading wildcard label (`*.rdeb.io` — subdomains only, not `rdeb.io`
  itself). Schemes, paths, ports, user info, a bare `*`, and wildcards over a
  single label (`*.com`) are rejected at load.
- **Enforcement:** the list is compiled when the plugin loads and bound into
  that plugin's host functions together with its id — JS cannot see or change
  it (mutating `module.exports.permissions` later has no effect). Every
  request **and every redirect hop** is checked; a host outside the list fails
  the call with `Request blocked: <host> is not in this plugin's
  permissions.domains`, exactly like an SSRF rejection. The SSRF guard
  (no private / loopback / link-local / metadata addresses) still applies on
  top.
- **Install:** the Web UI and `amigo-dl plugins install <id>` show the
  requested domains and require confirmation; the server refuses an install
  without `{"approve_permissions": true}` (HTTP 428).
- **Updates:** an update whose domain set is wider than the installed
  version's is never applied automatically — background auto-update skips it,
  and a manual update needs the same confirmation. If a plugin's own manifest
  claims more than what was approved, it is disabled after install/update.
- **No `permissions.domains`** means *unscoped*: the plugin may reach any
  public host (today's behaviour). Such plugins are flagged as "Unscoped" in
  the UI and installing them shows an explicit warning. Generic plugins such as
  `generic-http` are unscoped by necessity. The default will flip to
  deny-unless-declared once third-party plugins have migrated.

## Data Types

```typescript
interface DownloadPackage {
    name: string;                // Package name shown in UI
    downloads: DownloadInfo[];   // One or more files
}

interface DownloadInfo {
    url: string;
    filename: string | null;
    filesize: number | null;
    chunks_supported: boolean;
    max_chunks: number | null;
    headers: Record<string, string> | null;
    cookies: Record<string, string> | null;
    wait_seconds: number | null;
    mirrors: string[];
}

interface PostProcessContext {
    download_id: string;
    filename: string;
    filepath: string;
    filesize: number;
    mime_type: string | null;
    protocol: string;            // "http", "usenet", "hls", "dash"
    package_name: string;
    all_files: string[];
}

interface PostProcessResult {
    success: boolean;
    files_created?: string[];
    files_to_delete?: string[];
    message?: string;
}
```

## Host API Reference

All functions are available under the global `amigo.*` object.

### HTTP

All HTTP functions go through the sandbox proxy — no direct network access.

```typescript
// GET — returns parsed response object
amigo.httpGet(url: string, opts?: { headers?: Record<string, string> }): HttpResponse

// POST
amigo.httpPost(url: string, body: string, contentType: string,
               opts?: { headers?: Record<string, string> }): HttpResponse

// HEAD
amigo.httpHead(url: string, opts?: { headers?: Record<string, string> }): HeadResponse

// GET + auto-parse body as JSON (response.data contains parsed body)
amigo.httpGetJson(url: string, opts?: { headers?: Record<string, string> }): HttpJsonResponse
```

Response types:
```typescript
interface HttpResponse { status: number; body: string; headers: Record<string, string> }
interface HttpJsonResponse extends HttpResponse { data: any }
interface HeadResponse { status: number; headers: Record<string, string> }
```

**Examples:**
```typescript
const page = amigo.httpGet("https://example.com");
// page.status === 200, page.body === "<html>...", page.headers["content-type"] === "text/html"

const api = amigo.httpGetJson("https://api.example.com/data");
// api.data.items[0].name — already parsed JSON

const resp = amigo.httpPost(url, JSON.stringify({key: "val"}), "application/json", {
    headers: { "Authorization": "Bearer token123" }
});
```

### URL Helpers

```typescript
amigo.urlResolve(base: string, relative: string): string
amigo.urlParse(url: string): ParsedUrl
amigo.urlFilename(url: string): string | null
```

```typescript
amigo.urlResolve("https://example.com/page/", "../file.zip")
// → "https://example.com/file.zip"

amigo.urlParse("https://example.com:8080/path?q=1#hash")
// → { protocol: "https", host: "example.com", port: 8080,
//     pathname: "/path", search: "?q=1", hash: "#hash",
//     origin: "https://example.com:8080" }

amigo.urlFilename("https://cdn.example.com/files/document%20v2.pdf?token=abc")
// → "document v2.pdf"
```

### HTML Helpers

Powered by CSS selectors (Rust `scraper` crate). No fragile regex needed.

```typescript
amigo.htmlQueryAll(html: string, selector: string): string[]
amigo.htmlQueryText(html: string, selector: string): string | null
amigo.htmlQueryAttr(html: string, selector: string, attr: string): string | null
amigo.htmlSearchMeta(html: string, names: string | string[]): string | null
amigo.htmlExtractTitle(html: string): string | null
amigo.htmlHiddenInputs(html: string): Record<string, string>
amigo.searchJson(startPattern: string, html: string): any | null
```

**Examples:**
```typescript
// Get all download links
const links = amigo.htmlQueryAll(html, "a.download-link");

// Get video URL from OpenGraph meta
const videoUrl = amigo.htmlSearchMeta(html, ["og:video:url", "og:video", "twitter:player"]);

// Extract title
const title = amigo.htmlExtractTitle(html);

// Get hidden form fields (useful for login forms)
const inputs = amigo.htmlHiddenInputs(html);
// → { "csrf_token": "abc123", "action": "download" }

// Find JSON embedded in a <script> tag
const config = amigo.searchJson("window\\.config\\s*=\\s*", html);
// → { apiUrl: "...", videoId: "..." }
```

### Regex Helpers

```typescript
amigo.regexMatch(pattern: string, text: string): string | null
amigo.regexMatchAll(pattern: string, text: string): string[]
amigo.regexReplace(pattern: string, text: string, replacement: string): string | null
amigo.regexTest(pattern: string, text: string): boolean
amigo.regexSplit(pattern: string, text: string): string[]
```

`regexMatch` returns the first capture group, or the full match if no groups.
`regexMatchAll` returns all first capture groups.

### Utility

```typescript
// Parse duration: "1:23:45", "12:34", "PT1H23M45S" → seconds
amigo.parseDuration(input: string): number | null

// Remove invalid filename characters
amigo.sanitizeFilename(name: string): string

// Safe deep property access (returns null if path doesn't exist)
amigo.traverse(obj: any, path: string | (string | number)[]): any | null
```

```typescript
amigo.parseDuration("1:23:45")    // → 5025
amigo.parseDuration("PT2H30M")    // → 9000
amigo.sanitizeFilename('My Video: "Best" <2024>')  // → "My Video_ _Best_ _2024_"
amigo.traverse(data, "streamingData.formats.0.url")  // → string or null
```

### Cookies

```typescript
amigo.setCookie(domain: string, name: string, value: string): void
amigo.getCookie(domain: string, name: string): string | null
amigo.clearCookies(domain: string): void
```

### Persistent Storage

Per-plugin key-value storage that survives restarts.

```typescript
amigo.storageGet(key: string): string | null
amigo.storageSet(key: string, value: string): void
amigo.storageDelete(key: string): void
```

### Encoding

```typescript
amigo.base64Encode(input: string): string
amigo.base64Decode(input: string): string
```

### Crypto

```typescript
amigo.md5(input: string): string              // hex-encoded
amigo.sha1(input: string): string             // hex-encoded
amigo.sha256(input: string): string           // hex-encoded
amigo.hmacSha256(key: string, data: string): string  // hex-encoded
amigo.aesDecryptCbc(data: string, key: string, iv: string): string  // base64 in/out, hex key/iv
amigo.aesEncryptCbc(data: string, key: string, iv: string): string  // base64 in/out, hex key/iv
```

AES uses 128-bit keys (16 bytes = 32 hex chars) with PKCS7 padding.

### Captcha

```typescript
// Blocks until the user solves the captcha via the Web UI, or timeout (5 min).
// Throws on timeout or if the user clicks "Skip".
amigo.solveCaptcha(imageUrl: string, captchaType?: "image" | "recaptcha" | "hcaptcha"): string
```

The captcha image is displayed in a dialog in the Web UI. The user types the solution
and the plugin receives it as the return value. This is the same pattern as JDownloader's
manual captcha solving.

### Notifications

```typescript
// Sends a toast notification to the Web UI + triggers webhooks.
amigo.notify(title: string, message: string): void
```

### Logging

```typescript
amigo.logInfo(msg: string): void
amigo.logWarn(msg: string): void
amigo.logError(msg: string): void
amigo.logDebug(msg: string): void
```

Also available as `console.log()`, `console.warn()`, `console.error()`.

## Sandbox Limits

Each plugin runs in **its own QuickJS runtime**, so memory and the execution
deadline are per plugin; different plugins execute concurrently on the
server's blocking thread pool, and one slow plugin never delays another or
the plugin list.

| Limit | Default | Enforced how |
|-------|---------|--------------|
| Execution timeout | 30 s per `resolve()` / `postProcess()` call | Interrupt handler; also polled inside the regex engine, so a catastrophically backtracking native `RegExp` is stopped too |
| Load timeout | 5 s for evaluating the module body and reading its exports (incl. getters) | Same interrupt handler — a top-level `while (true) {}` fails to load instead of hanging startup |
| Memory | 64 MB per plugin | QuickJS runtime memory limit (per plugin runtime) |
| Stack | 1 MB per plugin | QuickJS max stack size |
| HTTP requests | 20 per invocation | Per-plugin counter in the host API |
| Network destinations | `permissions.domains`, if declared | Checked on every request and redirect hop |
| Storage | 1 MB per plugin | Host-side quota |

**What these limits are not:** they bound a *cooperating* interpreter. Plugin
JS still runs inside the daemon process, so a memory-safety bug in QuickJS
itself is not contained by any of them. Out-of-process isolation is tracked in
#79. The execution deadline is wall-clock for the whole invocation: it cannot
interrupt a pending host call (an HTTP request has its own 30 s timeout;
`amigo.solveCaptcha` waits for the user), but JS is stopped as soon as it
resumes past the deadline.

In addition, the Host API caps its inputs *outside* the QuickJS context — the
64 MB JS memory limit doesn't protect the host heap, so oversized arguments
are rejected with an error instead of being processed:

| Host API input | Cap |
|----------------|-----|
| Regex pattern (`regex*`) | 4 KiB |
| Regex haystack (`regex*`) | 4 MiB |
| Compiled regex NFA / DFA cache | 1 MiB / 2 MiB |
| HTML passed to `htmlQuery*` / `htmlSearchMeta` / … | 8 MiB |
| Base64 input (`base64Decode`) | 8 MiB |

No direct network, filesystem, or process access. Everything is proxied through the Host API.

## Compiled bytecode is local-only

To make restarts cheap, the server caches each plugin's compiled QuickJS
bytecode on disk (`$AMIGO_CONFIG_DIR/cache/plugin-bytecode`). QuickJS does
**not** validate bytecode before executing it — malformed or version-mismatched
bytecode is a memory-safety problem, not a parse error — so:

- bytecode is only ever produced locally, from source that passed the loader's
  checks; the registry serves `.ts`/`.js` source only and any other artifact is
  rejected;
- the cache key includes the QuickJS version and a cache-format version, so an
  engine upgrade is a cache miss, never a load of stale bytecode;
- every entry carries a SHA-256 of its payload; a truncated or corrupted entry
  is deleted and the plugin is recompiled from source;
- the cache directory must only be writable by the daemon (plugin JS has no
  filesystem access at all).

## Reloading Plugins

Plugins are loaded from the plugin directories at server startup. Loading a
plugin whose `id` is already registered replaces the previous version, which
is how plugin updates (`POST /api/v1/updates/plugins/{id}`) take effect
without a restart. There is no filesystem watcher yet — after editing a
plugin file on disk, restart the server (or re-trigger a load via the update
endpoint) to pick up the change. For fast iteration during development, use
`amigo-dl plugins test <plugin.ts> [url]`, which loads the file fresh on
every run.

## Directory Structure

```
plugins/
├── types/
│   └── amigo.d.ts           # TypeScript type definitions
├── template/
│   └── plugin.ts            # Starter template
├── hosters/
│   └── generic-http/
│       ├── plugin.ts        # Plugin source
│       └── plugin.spec.ts   # Tests
└── extractors/
    └── youtube/
        ├── plugin.ts
        └── plugin.spec.ts
```

## Testing

Place a `plugin.spec.ts` next to your `plugin.ts`. Available test helpers:

```typescript
test("description", () => {
    // test body
});

assert(condition, "message");
assertEqual(actual, expected, "message");
assertNotNull(value, "message");
```

Run via the plugin loader's `run_spec()` method.
