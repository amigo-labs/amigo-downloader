/// <reference path="../types/amigo.d.ts" />

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

const DOWNLOAD_INDICATORS = [
    "download", "dl", "get", "fetch", "mirror", "direct",
    "cdn", "releases", "attachments", "files",
];

function isDirectFileUrl(url: string): boolean {
    const path = url.split("?")[0].toLowerCase();
    return FILE_EXTENSIONS.some(ext => path.endsWith(ext));
}

function filenameFromContentDisposition(header: string): string | null {
    return amigo.regexMatch('filename\\*?=["\']?(?:UTF-8\'\')?([^"\'\\s;]+)', header);
}

function filenameFromUrl(url: string): string | null {
    const path = url.split("?")[0];
    const segment = amigo.regexMatch('/([^/]+)$', path);
    if (segment && segment.includes(".")) return decodeURIComponent(segment);
    return null;
}

function scoreLink(href: string, text: string): number {
    let score = 0;
    const hrefLower = href.toLowerCase();
    const textLower = text.toLowerCase();

    if (isDirectFileUrl(href)) score += 50;

    for (const indicator of DOWNLOAD_INDICATORS) {
        if (hrefLower.includes(indicator)) score += 10;
        if (textLower.includes(indicator)) score += 15;
    }

    if (hrefLower.includes("javascript:")) score -= 100;
    if (hrefLower === "#" || hrefLower === "") score -= 100;
    if (hrefLower.includes("login") || hrefLower.includes("signup")) score -= 50;
    if (hrefLower.includes("facebook") || hrefLower.includes("twitter")) score -= 50;
    if (hrefLower.includes("mailto:")) score -= 100;

    if (/download\s*(now|here|file)?/i.test(textLower)) score += 30;
    if (/click\s*here\s*to\s*download/i.test(textLower)) score += 25;
    if (/get\s*(file|download)/i.test(textLower)) score += 20;
    if (/\.\w{2,4}\s*\([\d.]+ ?(KB|MB|GB)\)/i.test(textLower)) score += 25;

    return score;
}

function resolveRelativeUrl(href: string, pageUrl: string): string {
    if (href.startsWith("http")) return href;
    if (href.startsWith("/")) {
        const origin = amigo.regexMatch("(https?://[^/]+)", pageUrl);
        return origin ? origin + href : href;
    }
    return pageUrl.replace(/[^/]*$/, "") + href;
}

/** Extract page title from HTML. */
function extractTitle(html: string): string {
    return amigo.regexMatch("<title[^>]*>([^<]+)</title>", html) || "Download";
}

/** Probe a URL via HEAD and build a DownloadInfo. */
function probeUrl(url: string): DownloadInfo {
    const head: HeadResponse = JSON.parse(amigo.httpHead(url));
    let filename: string | null = null;
    let filesize: number | null = null;
    let acceptsRanges = false;

    if (head.headers["content-disposition"]) {
        filename = filenameFromContentDisposition(head.headers["content-disposition"]);
    }
    if (!filename) filename = filenameFromUrl(url);
    if (head.headers["content-length"]) {
        filesize = parseInt(head.headers["content-length"], 10) || null;
    }
    if (head.headers["accept-ranges"] === "bytes") acceptsRanges = true;

    return {
        url, filename, filesize,
        chunks_supported: acceptsRanges,
        max_chunks: 8,
        headers: null, cookies: null, wait_seconds: null, mirrors: [],
    };
}

interface ScoredLink { href: string; text: string; score: number }

/** Find all download-worthy links on an HTML page, scored and sorted. */
function findDownloadLinks(html: string, pageUrl: string): ScoredLink[] {
    const allHrefs = amigo.regexMatchAll('href=["\']([^"\']+)["\']', html);
    const allTexts = amigo.regexMatchAll('<a[^>]+href=["\'][^"\']+["\'][^>]*>([^<]*)</a>', html);

    const links: ScoredLink[] = [];
    const seen = new Set<string>();

    for (let i = 0; i < allHrefs.length; i++) {
        const href = resolveRelativeUrl(allHrefs[i], pageUrl);
        const text = (i < allTexts.length ? allTexts[i] : "") || "";
        const score = scoreLink(href, text);

        if (score >= 20 && !seen.has(href)) {
            seen.add(href);
            links.push({ href, text, score });
        }
    }

    links.sort((a, b) => b.score - a.score);
    return links;
}

module.exports = {
    id: "generic-http",
    name: "Generic HTTP",
    version: "3.0.0",
    urlPattern: "https?://.+",

    resolve(url: string): DownloadPackage {
        // Step 1: HEAD to check if URL is a direct file download
        const head: HeadResponse = JSON.parse(amigo.httpHead(url));
        const contentType = head.headers["content-type"] || "";
        const isHtml = contentType.includes("text/html");
        const hasContentDisposition = !!head.headers["content-disposition"];

        if (!isHtml || hasContentDisposition || isDirectFileUrl(url)) {
            const dl = probeUrl(url);
            return {
                name: dl.filename || filenameFromUrl(url) || "Download",
                downloads: [dl],
            };
        }

        // Step 2: HTML page — find all download links
        amigo.logInfo("Page is HTML, scanning for download links...");
        const page: HttpResponse = JSON.parse(amigo.httpGet(url));
        const title = extractTitle(page.body);
        const links = findDownloadLinks(page.body, url);

        if (links.length === 0) {
            throw new Error("No download links found on page: " + url);
        }

        amigo.logInfo("Found " + links.length + " download link(s)");

        // Probe each link for metadata
        const downloads: DownloadInfo[] = links.map(link => {
            amigo.logDebug("  → " + link.href + " (score: " + link.score + ")");
            return probeUrl(link.href);
        });

        return { name: title, downloads };
    },

    checkOnline(url: string): "online" | "offline" | "unknown" {
        const resp: HeadResponse = JSON.parse(amigo.httpHead(url));
        if (resp.status === 200) return "online";
        if (resp.status === 404) return "offline";
        return "unknown";
    },
} satisfies AmigoPlugin;
