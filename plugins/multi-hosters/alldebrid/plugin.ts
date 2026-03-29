/// <reference path="../../types/amigo.d.ts" />

// AllDebrid Multi-Hoster Plugin
// Converts premium hoster links to direct download links via AllDebrid API.

const API_BASE = "https://api.alldebrid.com/v4";

const SUPPORTED_DOMAINS = [
    "1fichier.com", "rapidgator.net", "uploaded.net", "uploaded.to",
    "turbobit.net", "nitroflare.com", "filefactory.com", "ddownload.com",
    "katfile.com", "uploadgig.com", "alfafile.net", "k2s.cc",
    "keep2share.cc", "fboom.me", "tezfiles.com", "hitfile.net",
    "file.al", "clicknupload.co", "uptobox.com", "mega.nz",
    "mediafire.com", "filestore.to", "hexupload.net",
];

function buildUrlPattern(): string {
    const escaped = SUPPORTED_DOMAINS.map(d => d.replace(/\./g, "\\."));
    return "https?://(?:www\\.)?" + "(?:" + escaped.join("|") + ")/.+";
}

function getApiKey(): string {
    const key = amigo.storageGet("api_key");
    if (!key) {
        throw new Error(
            "AllDebrid API key not configured. Go to Settings → Plugins → AllDebrid and enter your API key."
        );
    }
    return key;
}

module.exports = {
    id: "alldebrid",
    name: "AllDebrid",
    version: "1.0.0",
    description: "Premium link generator — unrestricts links from 70+ file hosters",
    author: "amigo-labs",
    urlPattern: buildUrlPattern(),
    pluginType: "multi-hoster" as PluginTypeHint,

    resolve(url: string): DownloadPackage {
        const apiKey = getApiKey();

        const resp = amigo.httpGet(
            API_BASE + "/link/unlock?agent=amigo-downloader&apikey=" + encodeURIComponent(apiKey) + "&link=" + encodeURIComponent(url)
        );

        if (resp.status !== 200) {
            throw new Error("AllDebrid API error (" + resp.status + "): " + resp.body);
        }

        const data = JSON.parse(resp.body);

        if (data.status !== "success" || !data.data || !data.data.link) {
            const errMsg = data.error ? data.error.message : "unknown error";
            throw new Error("AllDebrid could not resolve: " + url + " — " + errMsg);
        }

        const result = data.data;

        return {
            name: result.filename || "Download",
            downloads: [{
                url: result.link,
                filename: result.filename || null,
                filesize: result.filesize || null,
                chunks_supported: true,
                max_chunks: null,
                headers: null,
                cookies: null,
                wait_seconds: result.delayed ? 5 : null,
                mirrors: [],
            }],
        };
    },

    supportsPremium(): boolean {
        return true;
    },

    login(username: string, password: string): boolean {
        amigo.storageSet("api_key", password);
        amigo.logInfo("AllDebrid API key saved");

        const resp = amigo.httpGet(
            API_BASE + "/user?agent=amigo-downloader&apikey=" + encodeURIComponent(password)
        );
        if (resp.status === 200) {
            const data = JSON.parse(resp.body);
            if (data.status === "success") {
                amigo.logInfo("Authenticated as: " + data.data.user.username + " (Premium: " + data.data.user.isPremium + ")");
                return true;
            }
        }
        amigo.logError("AllDebrid authentication failed: " + resp.body);
        return false;
    },

    checkOnline(url: string): "online" | "offline" | "unknown" {
        const apiKey = amigo.storageGet("api_key");
        if (!apiKey) return "unknown";

        const resp = amigo.httpGet(
            API_BASE + "/link/infos?agent=amigo-downloader&apikey=" + encodeURIComponent(apiKey) + "&link[]=" + encodeURIComponent(url)
        );

        if (resp.status === 200) {
            const data = JSON.parse(resp.body);
            if (data.status === "success" && data.data && data.data.infos) {
                const info = data.data.infos[0];
                if (info && !info.error) return "online";
                return "offline";
            }
        }
        return "unknown";
    },
} satisfies AmigoPlugin;
