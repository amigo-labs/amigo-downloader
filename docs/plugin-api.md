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
    urlPattern: string;          // Regex matching URLs this plugin handles
    resolve(url: string): DownloadPackage;

    // ── Optional ──
    description?: string;
    author?: string;
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

| Limit | Default |
|-------|---------|
| Execution timeout | 30 seconds per `resolve()` call |
| Memory | 64 MB per plugin |
| HTTP requests | 20 per invocation |
| Storage | 1 MB per plugin |

No direct network, filesystem, or process access. Everything is proxied through the Host API.

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
