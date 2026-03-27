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

    /** Resolve a URL to a direct download. Called by the download engine. */
    resolve(url: string): DownloadInfo;

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
}

// ─── Data Types ─────────────────────────────────────────────────────

/** Returned by resolve() — tells the download engine what to download. */
interface DownloadInfo {
    /** Direct download URL. */
    url: string;
    /** Suggested filename. */
    filename: string;
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

/** HEAD response returned by amigo.httpHead. */
interface HeadResponse {
    status: number;
    headers: Record<string, string>;
}

// ─── Host API ───────────────────────────────────────────────────────

/** Host API injected by the runtime. Available as `amigo.*` in all plugins. */
declare const amigo: {
    // ── Network (all requests go through the sandbox) ──

    /** HTTP GET — returns JSON string, parse with JSON.parse(). */
    httpGet(url: string): string;
    /** HTTP POST — returns JSON string, parse with JSON.parse(). */
    httpPost(url: string, body: string, contentType: string): string;
    /** HTTP HEAD — returns JSON string, parse with JSON.parse(). */
    httpHead(url: string): string;

    // ── Cookies ──

    setCookie(domain: string, name: string, value: string): void;
    getCookie(domain: string, name: string): string | null;
    clearCookies(domain: string): void;

    // ── Parsing helpers ──

    /** Returns first capture group, or full match if no groups. */
    regexMatch(pattern: string, text: string): string | null;
    /** Returns all capture groups. */
    regexMatchAll(pattern: string, text: string): string[];
    base64Decode(input: string): string;
    base64Encode(input: string): string;

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

declare var module: { exports: AmigoPlugin };
