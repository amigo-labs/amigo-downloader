/// <reference path="../../types/amigo.d.ts" />

// Premiumize Multi-Hoster Plugin
// Converts premium hoster links to direct download links via Premiumize API.

const API_BASE = "https://www.premiumize.me/api";

const SUPPORTED_DOMAINS = [
    "1fichier.com", "rapidgator.net", "uploaded.net", "uploaded.to",
    "turbobit.net", "nitroflare.com", "filefactory.com", "ddownload.com",
    "katfile.com", "uploadgig.com", "alfafile.net", "k2s.cc",
    "keep2share.cc", "fboom.me", "tezfiles.com", "hitfile.net",
    "file.al", "clicknupload.co", "uptobox.com",
];

function buildUrlPattern(): string {
    const escaped = SUPPORTED_DOMAINS.map(d => d.replace(/\./g, "\\."));
    return "https?://(?:www\\.)?" + "(?:" + escaped.join("|") + ")/.+";
}

function getApiKey(): string {
    const key = amigo.storageGet("api_key");
    if (!key) {
        throw new Error(
            "Premiumize API key not configured. Go to Settings → Plugins → Premiumize and enter your API key."
        );
    }
    return key;
}

module.exports = {
    id: "premiumize",
    name: "Premiumize",
    version: "1.0.0",
    description: "Premium link generator — unrestricts links from 60+ file hosters",
    author: "amigo-labs",
    urlPattern: buildUrlPattern(),
    pluginType: "multi-hoster" as PluginTypeHint,

    resolve(url: string): DownloadPackage {
        const apiKey = getApiKey();

        const resp = amigo.httpPostForm(
            API_BASE + "/transfer/directdl",
            { src: url },
            { headers: { "Authorization": "Bearer " + apiKey } },
        );

        if (resp.status !== 200) {
            throw new Error("Premiumize API error (" + resp.status + "): " + resp.body);
        }

        const data = JSON.parse(resp.body);

        if (data.status !== "success" || !data.content || data.content.length === 0) {
            throw new Error("Premiumize could not resolve: " + url + " — " + (data.message || "unknown error"));
        }

        const downloads: DownloadInfo[] = data.content.map((item: any) => ({
            url: item.link,
            filename: item.path ? item.path.split("/").pop() : null,
            filesize: item.size || null,
            chunks_supported: true,
            max_chunks: null,
            headers: null,
            cookies: null,
            wait_seconds: null,
            mirrors: [],
        }));

        return {
            name: data.content[0].path ? data.content[0].path.split("/").pop() : "Download",
            downloads,
        };
    },

    supportsPremium(): boolean {
        return true;
    },

    login(username: string, password: string): boolean {
        amigo.storageSet("api_key", password);
        amigo.logInfo("Premiumize API key saved");

        const resp = amigo.httpGet(API_BASE + "/account/info", {
            headers: { "Authorization": "Bearer " + password },
        });
        if (resp.status === 200) {
            const user = JSON.parse(resp.body);
            if (user.status === "success") {
                amigo.logInfo("Authenticated as: " + user.customer_id + " (Premium until: " + user.premium_until + ")");
                return true;
            }
        }
        amigo.logError("Premiumize authentication failed: " + resp.body);
        return false;
    },

    checkOnline(url: string): "online" | "offline" | "unknown" {
        const apiKey = amigo.storageGet("api_key");
        if (!apiKey) return "unknown";

        const resp = amigo.httpPostForm(
            API_BASE + "/transfer/directdl",
            { src: url },
            { headers: { "Authorization": "Bearer " + apiKey } },
        );

        if (resp.status === 200) {
            const data = JSON.parse(resp.body);
            return data.status === "success" ? "online" : "offline";
        }
        return "unknown";
    },
} satisfies AmigoPlugin;
