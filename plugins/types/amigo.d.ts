// Type declarations for amigo-downloader plugin API.
// Include in your plugin project for IDE autocomplete and type checking.

// ─── Plugin Contract ────────────────────────────────────────────────

/** The object your plugin must assign to `module.exports`. */
interface AmigoPlugin {
    // ── Required properties ──

    /** Unique plugin identifier (e.g. "mega-nz"). */
    id: string;
    /** Human-readable plugin name (e.g. "MEGA.nz"). */
    name: string;
    /** Semver version string (e.g. "1.0.0"). */
    version: string;
    /** Regex pattern matching URLs this plugin handles. */
    urlPattern: string;

    // ── Required functions ──

    /** Resolve a URL into a download package. Always returns a package
     *  with a name and one or more downloads — like JDownloader packages. */
    resolve(url: string): DownloadPackage;

    // ── Optional properties ──

    /** Short description of the plugin. */
    description?: string;
    /** Plugin author name. */
    author?: string;

    // ── Optional functions ──

    /** Check if a URL is still online. */
    checkOnline?(url: string): "online" | "offline" | "unknown";
    /** Login to a premium account. Return true on success. */
    login?(username: string, password: string): boolean;
    /** Whether this plugin supports premium accounts. */
    supportsPremium?(): boolean;
    /** Decrypt a link container (DLC, CCF, etc.). Return extracted URLs. */
    decryptContainer?(data: string): string[];
    /** Resolve a folder URL into individual file URLs. */
    resolveFolder?(url: string): string[];
    /** Post-processing hook called after download completion. */
    postProcess?(context: PostProcessContext): PostProcessResult;
}

// ─── Data Types ─────────────────────────────────────────────────────

/** A download package — groups related downloads together (like JDownloader packages). */
interface DownloadPackage {
    /** Package name shown in the UI (e.g. page title, folder name). */
    name: string;
    /** The downloads in this package. */
    downloads: DownloadInfo[];
}

/** A single downloadable file within a package. */
interface DownloadInfo {
    /** Direct download URL. */
    url: string;
    /** Suggested filename, or null to let the engine detect from URL/headers. */
    filename: string | null;
    /** File size in bytes, or null if unknown. */
    filesize: number | null;
    /** Whether the server supports chunked/parallel downloads. */
    chunks_supported: boolean;
    /** Max number of parallel chunks, or null for default. */
    max_chunks: number | null;
    /** Extra HTTP headers to send with the download request. */
    headers: Record<string, string> | null;
    /** Cookies to send with the download request. */
    cookies: Record<string, string> | null;
    /** Seconds to wait before starting the download (countdown). */
    wait_seconds: number | null;
    /** Alternative mirror URLs for failover. */
    mirrors: string[];
}

/** HTTP response returned by amigo.httpGet/httpPost. */
interface HttpResponse {
    status: number;
    body: string;
    headers: Record<string, string>;
}

/** Extended HTTP response with parsed JSON body, returned by amigo.httpGetJson. */
interface HttpJsonResponse extends HttpResponse {
    data: any;
}

/** HEAD response returned by amigo.httpHead. */
interface HeadResponse {
    status: number;
    headers: Record<string, string>;
}

/** Parsed URL components returned by amigo.urlParse. */
interface ParsedUrl {
    protocol: string;
    host: string;
    port: number | null;
    pathname: string;
    search: string;
    hash: string;
    origin: string;
}

/** Context passed to postProcess() after a download completes. */
interface PostProcessContext {
    download_id: string;
    filename: string;
    filepath: string;
    filesize: number;
    mime_type: string | null;
    protocol: string;
    package_name: string;
    all_files: string[];
}

/** Result returned by postProcess(). */
interface PostProcessResult {
    success: boolean;
    files_created?: string[];
    files_to_delete?: string[];
    message?: string;
}

/** Options for HTTP request functions. */
interface HttpRequestOptions {
    headers?: Record<string, string>;
}

// ─── Host API ───────────────────────────────────────────────────────

/** Host API injected by the runtime. Available as `amigo.*` in all plugins. */
declare const amigo: {
    // ── Network (all requests go through the sandbox) ──

    /** HTTP GET — returns parsed response object directly. */
    httpGet(url: string, opts?: HttpRequestOptions): HttpResponse;
    /** HTTP POST — returns parsed response object directly. */
    httpPost(url: string, body: string, contentType: string, opts?: HttpRequestOptions): HttpResponse;
    /** HTTP HEAD — returns parsed response object directly. */
    httpHead(url: string, opts?: HttpRequestOptions): HeadResponse;
    /** HTTP GET + parse body as JSON — returns response with `data` field. */
    httpGetJson(url: string, opts?: HttpRequestOptions): HttpJsonResponse;

    // ── Cookies ──

    setCookie(domain: string, name: string, value: string): void;
    getCookie(domain: string, name: string): string | null;
    clearCookies(domain: string): void;

    // ── URL helpers ──

    /** Resolve a relative URL against a base URL. */
    urlResolve(base: string, relative: string): string;
    /** Parse a URL into components. */
    urlParse(url: string): ParsedUrl;
    /** Extract the filename from a URL path (URL-decoded). */
    urlFilename(url: string): string | null;

    // ── HTML helpers (CSS selectors via scraper crate) ──

    /** Query all elements matching a CSS selector, returns outer HTML strings. */
    htmlQueryAll(html: string, selector: string): string[];
    /** Get the text content of the first element matching a CSS selector. */
    htmlQueryText(html: string, selector: string): string | null;
    /** Get an attribute value from the first element matching a CSS selector. */
    htmlQueryAttr(html: string, selector: string, attr: string): string | null;
    /** Search meta tags by name or property, with fallback chain.
     *  Checks both `name` and `property` attributes (supports OpenGraph). */
    htmlSearchMeta(html: string, names: string | string[]): string | null;
    /** Extract the page title from <title> tag. */
    htmlExtractTitle(html: string): string | null;
    /** Extract all hidden input fields as name→value pairs. */
    htmlHiddenInputs(html: string): Record<string, string>;
    /** Find and parse JSON embedded in HTML/JavaScript (e.g. `var config = {...}`).
     *  `startPattern` is a regex that matches up to the opening `{` or `[`. */
    searchJson(startPattern: string, html: string): any | null;

    // ── Regex helpers ──

    /** Returns first capture group, or full match if no groups. */
    regexMatch(pattern: string, text: string): string | null;
    /** Returns all capture groups. */
    regexMatchAll(pattern: string, text: string): string[];
    /** Replace all matches of a pattern. */
    regexReplace(pattern: string, text: string, replacement: string): string | null;
    /** Test if a pattern matches. */
    regexTest(pattern: string, text: string): boolean;
    /** Split text by a pattern. */
    regexSplit(pattern: string, text: string): string[];

    // ── Encoding ──

    base64Decode(input: string): string;
    base64Encode(input: string): string;

    // ── Crypto ──

    /** MD5 hash, returns hex-encoded string. */
    md5(input: string): string;
    /** SHA-1 hash, returns hex-encoded string. */
    sha1(input: string): string;
    /** SHA-256 hash, returns hex-encoded string. */
    sha256(input: string): string;
    /** HMAC-SHA256, returns hex-encoded string. */
    hmacSha256(key: string, data: string): string;
    /** AES-128-CBC decrypt. Data and result are base64-encoded, key and IV are hex-encoded. */
    aesDecryptCbc(data: string, key: string, iv: string): string;
    /** AES-128-CBC encrypt. Data and result are base64-encoded, key and IV are hex-encoded. */
    aesEncryptCbc(data: string, key: string, iv: string): string;

    // ── Captcha ──

    /** Request manual captcha solving via the Web UI. Blocks until the user solves it or timeout.
     *  Returns the captcha solution text. Throws on timeout or cancellation. */
    solveCaptcha(imageUrl: string, captchaType?: "image" | "recaptcha" | "hcaptcha"): string;

    // ── Notifications ──

    /** Send a notification to the Web UI (shows as a toast). */
    notify(title: string, message: string): void;

    // ── Utility ──

    /** Parse a duration string into seconds. Supports "1:23:45", "12:34", and ISO 8601 "PT1H23M45S". */
    parseDuration(input: string): number | null;
    /** Sanitize a filename by replacing invalid characters with underscores. */
    sanitizeFilename(name: string): string;
    /** Safe deep property access. Returns null if any part of the path is missing.
     *  Path can be a dot-separated string or an array of keys. */
    traverse(obj: any, path: string | (string | number)[]): any | null;

    // ── Logging ──

    logInfo(msg: string): void;
    logWarn(msg: string): void;
    logError(msg: string): void;
    logDebug(msg: string): void;

    // ── Persistent storage (per plugin, survives restarts) ──

    storageGet(key: string): string | null;
    storageSet(key: string, value: string): void;
    storageDelete(key: string): void;
};

// ─── Module ─────────────────────────────────────────────────────────

declare var module: { exports: any };
