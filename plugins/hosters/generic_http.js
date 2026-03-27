// Generic HTTP fallback plugin — handles direct download URLs.

module.exports = {
    pluginId() { return "generic-http"; },
    pluginName() { return "Generic HTTP"; },
    pluginVersion() { return "1.0.0"; },
    urlPattern() { return "https?://.+"; },

    resolve(url) {
        var resp = JSON.parse(amigo.httpHead(url));
        var filename = "download";
        var filesize = null;
        var acceptsRanges = false;

        if (resp.headers) {
            if (resp.headers["content-disposition"]) {
                var m = amigo.regexMatch('filename="?([^"]+)"?', resp.headers["content-disposition"]);
                if (m) filename = m;
            }
            if (resp.headers["content-length"]) {
                filesize = parseInt(resp.headers["content-length"], 10) || null;
            }
            if (resp.headers["accept-ranges"] === "bytes") {
                acceptsRanges = true;
            }
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
    },

    checkOnline(url) {
        var resp = JSON.parse(amigo.httpHead(url));
        if (resp.status === 200) return "online";
        if (resp.status === 404) return "offline";
        return "unknown";
    },
};
