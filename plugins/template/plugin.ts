/// <reference path="../types/amigo.d.ts" />

module.exports = {
    id: "my-hoster",
    name: "My Hoster",
    version: "1.0.0",
    urlPattern: "https?://(www\\.)?my-hoster\\.com/.+",

    resolve(url: string): DownloadPackage {
        // TODO: implement
        throw new Error("Not implemented");
    },
} satisfies AmigoPlugin;
