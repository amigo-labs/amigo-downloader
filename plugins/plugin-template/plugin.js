// Plugin Template for amigo-downloader
// Copy this file and adapt it for your hoster.
//
// All network requests go through amigo.httpGet/httpPost/httpHead.
// Plugins do NOT have direct network or filesystem access.

module.exports = {
    pluginId() { return "my-hoster"; },
    pluginName() { return "My Hoster"; },
    pluginVersion() { return "1.0.0"; },

    urlPattern() {
        return "https?://(www\\.)?my-hoster\\.com/file/[a-zA-Z0-9]+";
    },

    resolve(url) {
        amigo.logInfo("Resolving: " + url);

        var resp = JSON.parse(amigo.httpGet(url));
        var downloadUrl = amigo.regexMatch('href="(https://dl\\.my-hoster\\.com/[^"]+)"', resp.body);

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
};
