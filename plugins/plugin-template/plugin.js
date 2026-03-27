module.exports = {
    id: "my-hoster",
    name: "My Hoster",
    version: "1.0.0",
    urlPattern: "https?://(www\\.)?my-hoster\\.com/.+",

    resolve(url) {
        // TODO: implement
        throw new Error("Not implemented");
    },
};
