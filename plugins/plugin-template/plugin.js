module.exports = {
    id: "my-hoster",
    name: "My Hoster",
    version: "1.0.0",
    urlPattern: "https?://(www\\.)?my-hoster\\.com/.+",

    resolve(url) {
        var resp = JSON.parse(amigo.httpGet(url));

        // TODO: extract download URL from page
        var downloadUrl = url;

        // TODO: extract filename, or null to let the engine detect it
        var filename = "download.bin";

        // TODO: extract filesize, or null if unknown
        var filesize = null;

        return {
            url: downloadUrl,
            filename: filename,
            filesize: filesize,
            chunks_supported: true,
            max_chunks: null,
            headers: null,
            cookies: null,
            wait_seconds: null,
            mirrors: [],
        };
    },
};
