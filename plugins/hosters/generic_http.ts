/// <reference path="../types/amigo.d.ts" />

// Common file extensions that indicate a direct download
const FILE_EXTENSIONS = [
    ".zip", ".rar", ".7z", ".tar", ".gz", ".bz2", ".xz", ".zst",
    ".exe", ".msi", ".dmg", ".pkg", ".deb", ".rpm", ".appimage",
    ".pdf", ".epub", ".mobi",
    ".iso", ".img",
    ".mp4", ".mkv", ".avi", ".mov", ".wmv", ".flv", ".webm",
    ".mp3", ".flac", ".wav", ".ogg", ".aac", ".m4a",
    ".jpg", ".jpeg", ".png", ".gif", ".webp", ".svg", ".bmp", ".tiff",
    ".doc", ".docx", ".xls", ".xlsx", ".ppt", ".pptx", ".odt", ".ods",
    ".apk", ".ipa",
    ".bin", ".dat", ".torrent", ".nzb",
];

// Patterns in URLs/link text that suggest a download action
const DOWNLOAD_INDICATORS = [
    "download", "dl", "get", "fetch", "mirror", "direct",
    "cdn", "releases", "attachments", "files",
];

/** Check if a URL path looks like a direct file download. */
function isDirectFileUrl(url: string): boolean {
    const path = url.split("?")[0].toLowerCase();
    return FILE_EXTENSIONS.some(ext => path.endsWith(ext));
}

/** Extract filename from Content-Disposition header. */
function filenameFromContentDisposition(header: string): string | null {
    return amigo.regexMatch('filename\\*?=["\']?(?:UTF-8\'\')?([^"\'\\s;]+)', header);
}

/** Extract filename from URL path. */
function filenameFromUrl(url: string): string | null {
    const path = url.split("?")[0];
    const segment = amigo.regexMatch('/([^/]+)$', path);
    if (segment && segment.includes(".")) {
        return decodeURIComponent(segment);
    }
    return null;
}

/** Score a link — higher = more likely to be the download link. */
function scoreLink(href: string, text: string): number {
    let score = 0;
    const hrefLower = href.toLowerCase();
    const textLower = text.toLowerCase();

    // Direct file URL is a strong signal
    if (isDirectFileUrl(href)) score += 50;

    // Download indicators in href or text
    for (const indicator of DOWNLOAD_INDICATORS) {
        if (hrefLower.includes(indicator)) score += 10;
        if (textLower.includes(indicator)) score += 15;
    }

    // Penalize navigation/social links
    if (hrefLower.includes("javascript:")) score -= 100;
    if (hrefLower === "#" || hrefLower === "") score -= 100;
    if (hrefLower.includes("login") || hrefLower.includes("signup")) score -= 50;
    if (hrefLower.includes("facebook") || hrefLower.includes("twitter")) score -= 50;
    if (hrefLower.includes("mailto:")) score -= 100;

    // Boost if text looks like a download button
    if (/download\s*(now|here|file)?/i.test(textLower)) score += 30;
    if (/click\s*here\s*to\s*download/i.test(textLower)) score += 25;
    if (/get\s*(file|download)/i.test(textLower)) score += 20;
    if (/\.\w{2,4}\s*\([\d.]+ ?(KB|MB|GB)\)/i.test(textLower)) score += 25;

    return score;
}

/** Find the best download link on an HTML page. */
function findDownloadLink(html: string, pageUrl: string): string | null {
    // Extract all <a href="...">text</a> pairs
    const hrefs = amigo.regexMatchAll('<a[^>]+href=["\']([^"\']+)["\'][^>]*>(.*?)</a>', html);

    let bestUrl: string | null = null;
    let bestScore = 0;

    // regexMatchAll returns capture group 1 — we need both href and text
    // Parse manually for full control
    const linkPattern = '<a[^>]+href=["\']([^"\']+)["\'][^>]*>([^<]*)</a>';
    const allHrefs = amigo.regexMatchAll('href=["\']([^"\']+)["\']', html);
    const allTexts = amigo.regexMatchAll('<a[^>]+href=["\'][^"\']+["\'][^>]*>([^<]*)</a>', html);

    for (let i = 0; i < allHrefs.length && i < allTexts.length; i++) {
        let href = allHrefs[i];
        const text = allTexts[i] || "";

        // Resolve relative URLs
        if (href.startsWith("/")) {
            const origin = amigo.regexMatch("(https?://[^/]+)", pageUrl);
            if (origin) href = origin + href;
        } else if (!href.startsWith("http")) {
            const base = pageUrl.replace(/[^/]*$/, "");
            href = base + href;
        }

        const score = scoreLink(href, text);
        if (score > bestScore) {
            bestScore = score;
            bestUrl = href;
        }
    }

    // Only return if we're reasonably confident
    return bestScore >= 20 ? bestUrl : null;
}

module.exports = {
    id: "generic-http",
    name: "Generic HTTP",
    version: "2.0.0",
    urlPattern: "https?://.+",

    resolve(url: string): DownloadInfo {
        // Step 1: HEAD request to check if URL is a direct download
        const head: HeadResponse = JSON.parse(amigo.httpHead(url));

        const contentType = head.headers["content-type"] || "";
        const isHtml = contentType.includes("text/html");
        const hasContentDisposition = !!head.headers["content-disposition"];

        // Direct file download (binary content or content-disposition)
        if (!isHtml || hasContentDisposition || isDirectFileUrl(url)) {
            let filename: string | null = null;
            let filesize: number | null = null;
            let acceptsRanges = false;

            if (head.headers["content-disposition"]) {
                filename = filenameFromContentDisposition(head.headers["content-disposition"]);
            }
            if (!filename) {
                filename = filenameFromUrl(url);
            }
            if (head.headers["content-length"]) {
                filesize = parseInt(head.headers["content-length"], 10) || null;
            }
            if (head.headers["accept-ranges"] === "bytes") {
                acceptsRanges = true;
            }

            return {
                url: url,
                filename: filename,
                filesize: filesize,
                chunks_supported: acceptsRanges,
                max_chunks: 8,
                headers: null,
                cookies: null,
                wait_seconds: null,
                mirrors: [],
            };
        }

        // Step 2: HTML page — try to find the download link
        amigo.logInfo("Page is HTML, scanning for download links...");
        const page: HttpResponse = JSON.parse(amigo.httpGet(url));

        const downloadUrl = findDownloadLink(page.body, url);
        if (!downloadUrl) {
            throw new Error("No download link found on page: " + url);
        }

        amigo.logInfo("Found download link: " + downloadUrl);

        // HEAD the resolved download URL for metadata
        const dlHead: HeadResponse = JSON.parse(amigo.httpHead(downloadUrl));

        let filename: string | null = null;
        let filesize: number | null = null;
        let acceptsRanges = false;

        if (dlHead.headers["content-disposition"]) {
            filename = filenameFromContentDisposition(dlHead.headers["content-disposition"]);
        }
        if (!filename) {
            filename = filenameFromUrl(downloadUrl);
        }
        if (dlHead.headers["content-length"]) {
            filesize = parseInt(dlHead.headers["content-length"], 10) || null;
        }
        if (dlHead.headers["accept-ranges"] === "bytes") {
            acceptsRanges = true;
        }

        return {
            url: downloadUrl,
            filename: filename,
            filesize: filesize,
            chunks_supported: acceptsRanges,
            max_chunks: 8,
            headers: null,
            cookies: null,
            wait_seconds: null,
            mirrors: [],
        };
    },

    checkOnline(url: string): "online" | "offline" | "unknown" {
        const resp: HeadResponse = JSON.parse(amigo.httpHead(url));
        if (resp.status === 200) return "online";
        if (resp.status === 404) return "offline";
        return "unknown";
    },
} satisfies AmigoPlugin;
