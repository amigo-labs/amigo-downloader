// Plugin Template for amigo-downloader (TypeScript)
// Copy this file and adapt it for your hoster.
//
// All network requests go through amigo.httpGet/httpPost/httpHead.
// Plugins do NOT have direct network or filesystem access.

module.exports = {
    id: "my-hoster",
    name: "My Hoster",
    version: "1.0.0",
    urlPattern: "https?://(www\\.)?my-hoster\\.com/file/[a-zA-Z0-9]+",

    resolve(url: string): DownloadInfo {
        amigo.logInfo("Resolving: " + url);

        const resp: HttpResponse = JSON.parse(amigo.httpGet(url));
        const downloadUrl = amigo.regexMatch('href="(https://dl\\.my-hoster\\.com/[^"]+)"', resp.body);

        if (!downloadUrl) throw new Error("Download URL not found");

        return {
            url: downloadUrl,
            filename: "file.bin",
            filesize: null,
            chunks_supported: true,
            max_chunks: 4,
            headers: null,
            cookies: null,
            wait_seconds: null,
            mirrors: [],
        };
    },
} satisfies AmigoPlugin;
