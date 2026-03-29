/// <reference path="../../types/amigo.d.ts" />

// Generic HTTP Plugin — enhanced with media detection.
// Acts as the ultimate fallback when no specific plugin matches.
//
// Detection pipeline:
// 1. Direct file URL / Content-Disposition → immediate download
// 2. Media detection: m3u8/mpd URLs, <video>/<source> tags, OG/Twitter meta
// 3. Embedded player detection: JW Player, Video.js
// 4. Download link scraping with scoring heuristics

const FILE_EXTENSIONS = [
    ".zip", ".rar", ".7z", ".tar", ".gz", ".bz2", ".xz", ".zst",
    ".exe", ".msi", ".dmg", ".pkg", ".deb", ".rpm", ".appimage",
    ".pdf", ".epub", ".mobi",
    ".iso", ".img",
    ".mp4", ".mkv", ".avi", ".mov", ".wmv", ".flv", ".webm", ".m4v",
    ".mp3", ".flac", ".wav", ".ogg", ".aac", ".m4a", ".opus",
    ".jpg", ".jpeg", ".png", ".gif", ".webp", ".svg", ".bmp", ".tiff",
    ".doc", ".docx", ".xls", ".xlsx", ".ppt", ".pptx", ".odt", ".ods",
    ".apk", ".ipa",
    ".bin", ".dat", ".torrent", ".nzb",
    ".m3u8", ".mpd",
];

const MEDIA_EXTENSIONS = [
    ".mp4", ".mkv", ".avi", ".mov", ".wmv", ".flv", ".webm", ".m4v",
    ".mp3", ".flac", ".wav", ".ogg", ".aac", ".m4a", ".opus",
];

const DOWNLOAD_INDICATORS = [
    "download", "dl", "get", "fetch", "mirror", "direct",
    "cdn", "releases", "attachments", "files",
];

function isDirectFileUrl(url: string): boolean {
    const path = url.split("?")[0].toLowerCase();
    return FILE_EXTENSIONS.some(ext => path.endsWith(ext));
}

function isMediaUrl(url: string): boolean {
    const path = url.split("?")[0].toLowerCase();
    return MEDIA_EXTENSIONS.some(ext => path.endsWith(ext))
        || path.endsWith(".m3u8") || path.endsWith(".mpd");
}

function protocolForUrl(url: string): DownloadProtocol {
    const path = url.split("?")[0].toLowerCase();
    if (path.endsWith(".m3u8")) return "hls";
    if (path.endsWith(".mpd")) return "dash";
    return "http";
}

function filenameFromContentDisposition(header: string): string | null {
    return amigo.regexMatch('filename\\*?=["\']?(?:UTF-8\'\')?([^"\'\\s;]+)', header);
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

    if (amigo.regexTest("download\\s*(now|here|file)?", textLower)) score += 30;
    if (amigo.regexTest("click\\s*here\\s*to\\s*download", textLower)) score += 25;
    if (amigo.regexTest("get\\s*(file|download)", textLower)) score += 20;
    if (amigo.regexTest("\\.\\w{2,4}\\s*\\([\\d.]+ ?(KB|MB|GB)\\)", textLower)) score += 25;

    return score;
}

/** Probe a URL via HEAD and build a DownloadInfo. */
function probeUrl(url: string, protocol?: DownloadProtocol): DownloadInfo {
    const head = amigo.httpHead(url);
    let filename: string | null = null;
    let filesize: number | null = null;
    let acceptsRanges = false;

    if (head.headers["content-disposition"]) {
        filename = filenameFromContentDisposition(head.headers["content-disposition"]);
    }
    if (!filename) filename = amigo.urlFilename(url);
    if (head.headers["content-length"]) {
        filesize = parseInt(head.headers["content-length"], 10) || null;
    }
    if (head.headers["accept-ranges"] === "bytes") acceptsRanges = true;

    return {
        url, filename, filesize,
        chunks_supported: acceptsRanges,
        max_chunks: 8,
        headers: null, cookies: null, wait_seconds: null, mirrors: [],
        protocol: protocol || protocolForUrl(url),
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
        let href = allHrefs[i];
        if (!href.startsWith("http")) {
            try { href = amigo.urlResolve(pageUrl, href); } catch { continue; }
        }
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

// --- Media Detection ---

/** Find media URLs in OpenGraph and Twitter meta tags. */
function findMetaMedia(html: string, pageUrl: string): DownloadInfo[] {
    const results: DownloadInfo[] = [];
    const seen = new Set<string>();

    // OpenGraph video tags
    const ogTags = ["og:video", "og:video:url", "og:video:secure_url"];
    for (const tag of ogTags) {
        const content = amigo.htmlSearchMeta(html, tag);
        if (content && isMediaUrl(content) && !seen.has(content)) {
            seen.add(content);
            let url = content;
            if (!url.startsWith("http")) {
                try { url = amigo.urlResolve(pageUrl, url); } catch { continue; }
            }
            results.push(probeUrl(url));
        }
    }

    // Twitter player stream
    const twitterStream = amigo.htmlSearchMeta(html, "twitter:player:stream");
    if (twitterStream && isMediaUrl(twitterStream) && !seen.has(twitterStream)) {
        seen.add(twitterStream);
        results.push(probeUrl(twitterStream));
    }

    return results;
}

/** Find media in <video> and <source> tags. */
function findHtml5Media(html: string, pageUrl: string): DownloadInfo[] {
    const results: DownloadInfo[] = [];
    const seen = new Set<string>();

    // <video src="..."> and <audio src="...">
    const videoSrcs = amigo.htmlQueryAllAttrs(html, "video[src]", "src");
    const audioSrcs = amigo.htmlQueryAllAttrs(html, "audio[src]", "src");

    for (const src of [...videoSrcs, ...audioSrcs]) {
        let url = src;
        if (!url.startsWith("http")) {
            try { url = amigo.urlResolve(pageUrl, url); } catch { continue; }
        }
        if (!seen.has(url)) {
            seen.add(url);
            results.push(probeUrl(url));
        }
    }

    // <source src="...">
    const sourceSrcs = amigo.htmlQueryAllAttrs(html, "video source, audio source", "src");
    for (const src of sourceSrcs) {
        let url = src;
        if (!url.startsWith("http")) {
            try { url = amigo.urlResolve(pageUrl, url); } catch { continue; }
        }
        if (!seen.has(url)) {
            seen.add(url);
            results.push(probeUrl(url));
        }
    }

    return results;
}

/** Find m3u8/mpd/mp4 URLs in JavaScript. */
function findScriptMedia(html: string): DownloadInfo[] {
    const results: DownloadInfo[] = [];
    const seen = new Set<string>();

    // HLS manifests
    const m3u8Urls = amigo.regexMatchAll('["\']?(https?://[^"\'\\s]+\\.m3u8(?:\\?[^"\'\\s]*)?)["\']?', html);
    for (const url of m3u8Urls) {
        if (!seen.has(url)) {
            seen.add(url);
            results.push({
                url, filename: null, filesize: null,
                chunks_supported: false, max_chunks: null,
                headers: null, cookies: null, wait_seconds: null, mirrors: [],
                protocol: "hls",
            });
        }
    }

    // DASH manifests
    const mpdUrls = amigo.regexMatchAll('["\']?(https?://[^"\'\\s]+\\.mpd(?:\\?[^"\'\\s]*)?)["\']?', html);
    for (const url of mpdUrls) {
        if (!seen.has(url)) {
            seen.add(url);
            results.push({
                url, filename: null, filesize: null,
                chunks_supported: false, max_chunks: null,
                headers: null, cookies: null, wait_seconds: null, mirrors: [],
                protocol: "dash",
            });
        }
    }

    return results;
}

/** Detect JW Player and extract media URLs. */
function findJWPlayerMedia(html: string): DownloadInfo[] {
    if (!html.includes("jwplayer") && !html.includes("jwDefaults") && !html.includes("jw-video")) {
        return [];
    }

    const results: DownloadInfo[] = [];
    const seen = new Set<string>();

    // file: "..." in jwplayer config
    const files = amigo.regexMatchAll('["\'\\s]file["\'\\s]*:\\s*["\'](https?://[^"\']+)["\']', html);
    for (const url of files) {
        if (isMediaUrl(url) && !seen.has(url)) {
            seen.add(url);
            results.push({
                url, filename: null, filesize: null,
                chunks_supported: url.endsWith(".mp4"),
                max_chunks: url.endsWith(".mp4") ? 8 : null,
                headers: null, cookies: null, wait_seconds: null, mirrors: [],
                protocol: protocolForUrl(url),
            });
        }
    }

    return results;
}

module.exports = {
    id: "generic-http",
    name: "Generic HTTP",
    version: "5.0.0",
    urlPattern: "https?://.+",
    pluginType: "generic" as PluginTypeHint,
    description: "Generic fallback — detects media and download links on any page",

    resolve(url: string): DownloadPackage {
        // Step 1: HEAD to check if URL is a direct file/media download
        const head = amigo.httpHead(url);
        const contentType = head.headers["content-type"] || "";
        const isHtml = contentType.includes("text/html");
        const hasContentDisposition = !!head.headers["content-disposition"];

        // Direct file or media URL
        if (!isHtml || hasContentDisposition || isDirectFileUrl(url)) {
            const dl = probeUrl(url);
            return {
                name: dl.filename || amigo.urlFilename(url) || "Download",
                downloads: [dl],
            };
        }

        // Step 2: Fetch the HTML page
        amigo.logInfo("Page is HTML, running detection pipeline...");
        const page = amigo.httpGet(url);
        const title = amigo.htmlExtractTitle(page.body) || "Download";

        // Step 3: Media detection pipeline (try each method)
        const mediaDownloads: DownloadInfo[] = [];

        // 3a: OpenGraph / Twitter meta tags
        mediaDownloads.push(...findMetaMedia(page.body, url));

        // 3b: HTML5 <video>/<audio>/<source>
        mediaDownloads.push(...findHtml5Media(page.body, url));

        // 3c: JW Player detection
        mediaDownloads.push(...findJWPlayerMedia(page.body));

        // 3d: Script URL mining (m3u8, mpd)
        mediaDownloads.push(...findScriptMedia(page.body));

        if (mediaDownloads.length > 0) {
            amigo.logInfo("Found " + mediaDownloads.length + " media stream(s)");
            return { name: title, downloads: mediaDownloads };
        }

        // Step 4: Fallback — download link scraping
        amigo.logInfo("No media found, scanning for download links...");
        const links = findDownloadLinks(page.body, url);

        if (links.length === 0) {
            throw new Error("No download links or media found on page: " + url);
        }

        amigo.logInfo("Found " + links.length + " download link(s)");

        const downloads: DownloadInfo[] = links.map(link => {
            amigo.logDebug("  > " + link.href + " (score: " + link.score + ")");
            return probeUrl(link.href);
        });

        return { name: title, downloads };
    },

    checkOnline(url: string): "online" | "offline" | "unknown" {
        const resp = amigo.httpHead(url);
        if (resp.status === 200) return "online";
        if (resp.status === 404) return "offline";
        return "unknown";
    },
} satisfies AmigoPlugin;
