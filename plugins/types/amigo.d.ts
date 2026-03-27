// Type declarations for amigo-downloader plugin API.
// Include in your plugin project for IDE autocomplete and type checking.

interface DownloadInfo {
    url: string;
    filename: string;
    filesize: number | null;
    chunks_supported: boolean;
    max_chunks: number | null;
    headers: Record<string, string> | null;
    cookies: Record<string, string> | null;
    wait_seconds: number | null;
    mirrors: string[];
}

interface HttpResponse {
    status: number;
    body: string;
    headers: Record<string, string>;
}

declare const amigo: {
    /** GET request — returns JSON string of HttpResponse. */
    httpGet(url: string): string;
    /** POST request — returns JSON string of HttpResponse. */
    httpPost(url: string, body: string, contentType: string): string;
    /** HEAD request — returns JSON string of {status, headers}. */
    httpHead(url: string): string;

    /** Set a cookie for a domain. */
    setCookie(domain: string, name: string, value: string): void;
    /** Get a cookie value. */
    getCookie(domain: string, name: string): string | null;
    /** Clear all cookies for a domain. */
    clearCookies(domain: string): void;

    /** Match first regex capture group (or full match). */
    regexMatch(pattern: string, text: string): string | null;
    /** Match all regex capture groups. */
    regexMatchAll(pattern: string, text: string): string[];

    /** Base64 decode. */
    base64Decode(input: string): string;
    /** Base64 encode. */
    base64Encode(input: string): string;

    /** Log info message. */
    logInfo(msg: string): void;
    /** Log warning message. */
    logWarn(msg: string): void;
    /** Log error message. */
    logError(msg: string): void;
    /** Log debug message. */
    logDebug(msg: string): void;

    /** Get persistent storage value. */
    storageGet(key: string): string | null;
    /** Set persistent storage value. */
    storageSet(key: string, value: string): void;
    /** Delete persistent storage value. */
    storageDelete(key: string): void;
};
