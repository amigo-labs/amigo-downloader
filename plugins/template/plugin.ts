/// <reference path="../types/amigo.d.ts" />

module.exports = {
    id: "my-hoster",
    name: "My Hoster",
    version: "1.0.0",
    description: "Plugin for my-hoster.com",
    author: "your-name",
    urlPattern: "https?://(www\\.)?my-hoster\\.com/.+",

    resolve(url: string): DownloadPackage {
        // Fetch the page
        const page = amigo.httpGet(url);
        if (page.status !== 200) {
            throw new Error("HTTP " + page.status);
        }

        // Extract download link using CSS selector or regex
        const directUrl = amigo.htmlQueryAttr(page.body, "a.download-btn", "href");
        if (!directUrl) {
            throw new Error("No download link found");
        }

        // Resolve relative URL
        const fullUrl = amigo.urlResolve(url, directUrl);
        const title = amigo.htmlExtractTitle(page.body) || "Download";

        // Probe the file for metadata
        const head = amigo.httpHead(fullUrl);
        const filename = amigo.urlFilename(fullUrl);
        const filesize = head.headers["content-length"]
            ? parseInt(head.headers["content-length"])
            : null;

        return {
            name: title,
            downloads: [{
                url: fullUrl,
                filename: filename,
                filesize: filesize,
                chunks_supported: head.headers["accept-ranges"] === "bytes",
                max_chunks: null,
                headers: null,
                cookies: null,
                wait_seconds: null,
                mirrors: [],
            }],
        };
    },

    // Optional: check if a URL is still available
    checkOnline(url: string): "online" | "offline" | "unknown" {
        const resp = amigo.httpHead(url);
        if (resp.status === 200) return "online";
        if (resp.status === 404) return "offline";
        return "unknown";
    },

    // Optional: post-processing after download completes
    // postProcess(ctx: PostProcessContext): PostProcessResult {
    //     amigo.logInfo("Post-processing: " + ctx.filename);
    //     return { success: true };
    // },
} satisfies AmigoPlugin;
