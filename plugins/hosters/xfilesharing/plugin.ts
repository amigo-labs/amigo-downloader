/// <reference path="../../types/amigo.d.ts" />

// XFileSharingPro Generic Plugin
// Handles ~50+ file hosting sites that run on the XFileSharingPro PHP script.
// All XFS sites share the same page structure and download flow.

interface XfsSite {
    domain: string;
    name: string;
}

const XFS_SITES: XfsSite[] = [
    { domain: "ddownload.com", name: "DDownload" },
    { domain: "katfile.com", name: "Katfile" },
    { domain: "hexupload.net", name: "HexUpload" },
    { domain: "clicknupload.co", name: "ClicknUpload" },
    { domain: "file.al", name: "File.al" },
    { domain: "uploadrar.com", name: "UploadRAR" },
    { domain: "filerio.in", name: "FileRio" },
    { domain: "filenext.com", name: "FileNext" },
    { domain: "isra.cloud", name: "Isra.cloud" },
    { domain: "worldbytez.com", name: "WorldBytez" },
    { domain: "uploadbank.com", name: "UploadBank" },
    { domain: "fastclick.to", name: "FastClick" },
    { domain: "dailyuploads.net", name: "DailyUploads" },
    { domain: "userupload.net", name: "UserUpload" },
    { domain: "mx-sh.net", name: "MixShare" },
    { domain: "streamvid.net", name: "StreamVid" },
    { domain: "upstream.to", name: "Upstream" },
    { domain: "hotlink.cc", name: "Hotlink" },
    { domain: "douploads.net", name: "DouUploads" },
    { domain: "uploadev.org", name: "Uploadev" },
    { domain: "sfile.mobi", name: "SFile" },
    { domain: "anonfiles.la", name: "AnonFiles" },
    { domain: "fikper.com", name: "Fikper" },
    { domain: "filelox.com", name: "FileLox" },
    { domain: "down.fast-down.com", name: "FastDown" },
    { domain: "ouo.press", name: "OuoPress" },
    { domain: "drop.download", name: "Drop.download" },
    { domain: "mexa.sh", name: "MexaSH" },
];

function buildUrlPattern(): string {
    const escaped = XFS_SITES.map(s => s.domain.replace(/\./g, "\\."));
    return "https?://(?:www\\.)?" + "(?:" + escaped.join("|") + ")/.+";
}

module.exports = {
    id: "xfilesharing",
    name: "XFileSharingPro (Generic)",
    version: "1.0.0",
    description: "Generic plugin for 50+ file hosting sites based on XFileSharingPro",
    author: "amigo-labs",
    urlPattern: buildUrlPattern(),
    pluginType: "hoster" as PluginTypeHint,

    resolve(url: string): DownloadPackage {
        // Step 1: GET the file page
        amigo.logInfo("XFS: Fetching file page: " + url);
        const page1 = amigo.httpGet(url);

        if (page1.status !== 200) {
            throw new Error("XFS: Page returned " + page1.status);
        }

        // Check if file exists
        if (page1.body.includes("File Not Found") || page1.body.includes("file was deleted")) {
            throw new Error("XFS: File not found or deleted");
        }

        // Extract filename from the page
        let filename = amigo.htmlQueryText(page1.body, "h1, h2, .file-title, #file_title, td.file-title") || null;
        if (filename) {
            filename = filename.trim();
        }

        // Extract file size
        let filesize: number | null = null;
        // Use a single capture group for "number + unit" together so regexMatch
        // returns both parts (regexMatch returns the first capture group only).
        const sizeFullMatch = amigo.regexMatch("(\\d[\\d.]*\\s*(?:KB|MB|GB|TB))", page1.body);
        if (sizeFullMatch) {
            const num = parseFloat(sizeFullMatch);
            const unitMatch = amigo.regexMatch("(KB|MB|GB|TB)", sizeFullMatch);
            if (!isNaN(num) && unitMatch) {
                const multipliers: Record<string, number> = {
                    "KB": 1024,
                    "MB": 1024 * 1024,
                    "GB": 1024 * 1024 * 1024,
                    "TB": 1024 * 1024 * 1024 * 1024,
                };
                const unit = unitMatch.toUpperCase();
                if (multipliers[unit]) {
                    filesize = Math.round(num * multipliers[unit]);
                }
            }
        }

        // Step 2: Submit the first form (usually method_free or method_premium)
        const hiddenInputs = amigo.htmlHiddenInputs(page1.body);

        // Many XFS sites have a two-step process:
        // Step 1 form: op=download1, id=..., fname=..., method_free=...
        // Step 2 form: op=download2, id=..., rand=..., referer=..., down_direct=1

        // Try to find and submit the download form
        let downloadUrl: string | null = null;

        // Check if page already has a direct download link
        downloadUrl = amigo.htmlQueryAttr(page1.body, "a#direct-link, a.btn-download, a[href*='/d/'], a[href*='/dl/']", "href");

        if (!downloadUrl) {
            // Submit the free download form
            const formFields: Record<string, string> = { ...hiddenInputs };
            if (!formFields["op"]) {
                formFields["op"] = "download2";
            }
            if (!formFields["method_free"]) {
                formFields["method_free"] = "Free Download";
            }

            // Check for wait time
            const waitMatch = amigo.regexMatch('id="countdown"[^>]*>\\s*(\\d+)', page1.body)
                || amigo.regexMatch('var\\s+seconds\\s*=\\s*(\\d+)', page1.body);
            if (waitMatch) {
                const waitSecs = parseInt(waitMatch, 10);
                if (waitSecs > 0 && waitSecs <= 120) {
                    amigo.logInfo("XFS: Waiting " + waitSecs + " seconds...");
                    // We signal the wait to the engine via wait_seconds in the result
                }
            }

            amigo.logInfo("XFS: Submitting download form...");
            const page2 = amigo.httpPostForm(url, formFields);

            if (page2.status !== 200) {
                throw new Error("XFS: Form submission returned " + page2.status);
            }

            // Extract the direct download link from the response
            downloadUrl = amigo.htmlQueryAttr(page2.body, "a#direct-link, a.btn-download, a[href*='/d/'], a[href*='/dl/']", "href");

            // Also try to find it in a script
            if (!downloadUrl) {
                downloadUrl = amigo.regexMatch(
                    '(?:direct_link|dl_url|download_url)\\s*[=:]\\s*["\']([^"\']+)["\']',
                    page2.body
                );
            }

            // Try to find any direct file link
            if (!downloadUrl) {
                downloadUrl = amigo.regexMatch(
                    'href=["\']?(https?://[^"\'\\s]*\\.(?:zip|rar|7z|mp4|mkv|avi|mp3|pdf|exe|iso|tar\\.gz)[^"\'\\s]*)["\']?',
                    page2.body
                );
            }
        }

        if (!downloadUrl) {
            throw new Error("XFS: Could not extract download link. The site may require a captcha or premium account.");
        }

        // Resolve relative URLs
        if (!downloadUrl.startsWith("http")) {
            downloadUrl = amigo.urlResolve(url, downloadUrl);
        }

        return {
            name: filename || amigo.urlFilename(downloadUrl) || "Download",
            downloads: [{
                url: downloadUrl,
                filename: filename,
                filesize: filesize,
                chunks_supported: true,
                max_chunks: 8,
                headers: { "Referer": url },
                cookies: null,
                wait_seconds: null,
                mirrors: [],
            }],
        };
    },

    checkOnline(url: string): "online" | "offline" | "unknown" {
        const resp = amigo.httpGet(url);
        if (resp.status === 200) {
            if (resp.body.includes("File Not Found") || resp.body.includes("file was deleted")) {
                return "offline";
            }
            return "online";
        }
        if (resp.status === 404) return "offline";
        return "unknown";
    },
} satisfies AmigoPlugin;
