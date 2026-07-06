/// <reference path="../../types/amigo.d.ts" />

// Real-Debrid Multi-Hoster Plugin
// Converts premium hoster links to direct download links via Real-Debrid API.
// One plugin replaces ~80 individual hoster plugins.

const API_BASE = "https://api.real-debrid.com/rest/1.0";

// Hosters supported by Real-Debrid (partial list — dynamically updated)
const SUPPORTED_DOMAINS = [
    "1fichier.com", "rapidgator.net", "uploaded.net", "uploaded.to",
    "turbobit.net", "nitroflare.com", "filefactory.com", "ddownload.com",
    "katfile.com", "megaupload.nz", "filestore.to", "hexupload.net",
    "uploadgig.com", "alfafile.net", "filerio.in", "filenext.com",
    "isra.cloud", "worldbytez.com", "k2s.cc", "keep2share.cc",
    "fboom.me", "tezfiles.com", "hitfile.net", "file.al",
    "clicknupload.co", "filespace.com", "uptobox.com",
];

function buildUrlPattern(): string {
    const escaped = SUPPORTED_DOMAINS.map(d => d.replace(/\./g, "\\."));
    return "https?://(?:www\\.)?" + "(?:" + escaped.join("|") + ")/.+";
}

function getApiKey(): string {
    const key = amigo.storageGet("api_key");
    if (!key) {
        throw new Error(
            "Real-Debrid API key not configured. Go to Settings → Plugins → Real-Debrid and enter your API key."
        );
    }
    return key;
}

// Wrap JSON.parse so a malformed API response surfaces as a clear, sourced
// error instead of a raw SyntaxError that obscures which call failed.
function parseApiJson(body: string, label: string): any {
    try {
        return JSON.parse(body);
    } catch (e) {
        throw new Error("Real-Debrid " + label + " returned invalid JSON: " + (e as Error).message);
    }
}

module.exports = {
    id: "real-debrid",
    name: "Real-Debrid",
    version: "1.0.0",
    description: "Premium link generator — unrestricts links from 80+ file hosters",
    author: "amigo-labs",
    urlPattern: buildUrlPattern(),
    pluginType: "multi-hoster" as PluginTypeHint,

    resolve(url: string): DownloadPackage {
        const apiKey = getApiKey();
        const authHeader = { headers: { "Authorization": "Bearer " + apiKey } };

        // Step 1: Unrestrict the link
        const resp = amigo.httpPostForm(
            API_BASE + "/unrestrict/link",
            { link: url },
            authHeader,
        );

        if (resp.status !== 200) {
            throw new Error("Real-Debrid API error (" + resp.status + "): " + resp.body);
        }

        const data = parseApiJson(resp.body, "/unrestrict/link");

        if (!data.download) {
            throw new Error("Real-Debrid could not generate download link for: " + url);
        }

        return {
            name: data.filename || "Download",
            downloads: [{
                url: data.download,
                filename: data.filename || null,
                filesize: data.filesize || null,
                chunks_supported: true,
                max_chunks: null,
                headers: null,
                cookies: null,
                wait_seconds: null,
                mirrors: data.alternative ? [data.alternative] : [],
            }],
        };
    },

    supportsPremium(): boolean {
        return true;
    },

    login(username: string, password: string): boolean {
        // Real-Debrid uses API keys, not username/password (passed as password,
        // username is ignored). Verify the key works BEFORE persisting it, so a
        // bad key isn't left in storage on failure and reused later.
        const resp = amigo.httpGet(API_BASE + "/user", {
            headers: { "Authorization": "Bearer " + password },
        });
        if (resp.status === 200) {
            const user = parseApiJson(resp.body, "/user");
            amigo.storageSet("api_key", password);
            amigo.logInfo("Authenticated as: " + user.username + " (Premium: " + user.type + ")");
            return true;
        }
        amigo.logError("Real-Debrid authentication failed: " + resp.body);
        return false;
    },

    checkOnline(url: string): "online" | "offline" | "unknown" {
        const apiKey = amigo.storageGet("api_key");
        if (!apiKey) return "unknown";

        const resp = amigo.httpPostForm(
            API_BASE + "/unrestrict/check",
            { link: url },
            { headers: { "Authorization": "Bearer " + apiKey } },
        );

        if (resp.status === 200) {
            const data = parseApiJson(resp.body, "/unrestrict/check");
            if (data.supported === 1) return "online";
            return "offline";
        }
        return "unknown";
    },
} satisfies AmigoPlugin;
