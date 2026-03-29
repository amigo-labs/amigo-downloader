/// <reference path="../../types/amigo.d.ts" />

function extractVideoId(url: string): string | null {
    let m = amigo.regexMatch("[?&]v=([a-zA-Z0-9_-]{11})", url);
    if (m) return m;
    m = amigo.regexMatch("youtu\\.be/([a-zA-Z0-9_-]{11})", url);
    if (m) return m;
    m = amigo.regexMatch("/(embed|shorts|v)/([a-zA-Z0-9_-]{11})", url);
    return m;
}

module.exports = {
    id: "youtube",
    name: "YouTube",
    version: "1.1.0",
    description: "Download videos from YouTube",
    author: "amigo-labs",
    urlPattern: "https?://(www\\.)?(youtube\\.com/(watch|shorts|embed)|youtu\\.be/)",

    resolve(url: string): DownloadPackage {
        const videoId = extractVideoId(url);
        if (!videoId) throw new Error("Could not extract video ID from URL");

        amigo.logInfo("Resolving YouTube video: " + videoId);

        const body = JSON.stringify({
            videoId: videoId,
            context: {
                client: {
                    clientName: "ANDROID_VR",
                    clientVersion: "1.65.10",
                    deviceMake: "Oculus",
                    deviceModel: "Quest 3",
                    androidSdkVersion: 32,
                    osName: "Android",
                    osVersion: "12L",
                    hl: "en",
                    gl: "US",
                },
            },
        });

        const resp = amigo.httpPost(
            "https://www.youtube.com/youtubei/v1/player?prettyPrint=false",
            body,
            "application/json"
        );

        if (resp.status !== 200) {
            throw new Error("YouTube API returned status " + resp.status);
        }

        const data = JSON.parse(resp.body);
        const title = amigo.traverse(data, "videoDetails.title") as string | null;
        if (!title) throw new Error("Could not get video title");

        let bestUrl: string | null = null;
        let bestQuality = "";
        let bestHeight = 0;
        let bestSize: number | null = null;
        let bestMime = "video/mp4";

        const formats = amigo.traverse(data, "streamingData.formats") as any[] | null;
        if (formats) {
            for (const fmt of formats) {
                if (fmt.url && fmt.height && fmt.height > bestHeight) {
                    bestUrl = fmt.url;
                    bestHeight = fmt.height;
                    bestQuality = fmt.qualityLabel || "unknown";
                    bestSize = fmt.contentLength ? parseInt(fmt.contentLength) : null;
                    bestMime = (fmt.mimeType || "video/mp4").split(";")[0];
                }
            }
        }

        if (!bestUrl) {
            const adaptive = amigo.traverse(data, "streamingData.adaptiveFormats") as any[] | null;
            if (adaptive) {
                for (const afmt of adaptive) {
                    if (afmt.url && afmt.mimeType && afmt.mimeType.indexOf("video/") === 0) {
                        if (afmt.height && afmt.height > bestHeight) {
                            bestUrl = afmt.url;
                            bestHeight = afmt.height;
                            bestQuality = afmt.qualityLabel || "unknown";
                            bestSize = afmt.contentLength ? parseInt(afmt.contentLength) : null;
                            bestMime = afmt.mimeType.split(";")[0];
                        }
                    }
                }
            }
        }

        if (!bestUrl) {
            const reason = amigo.traverse(data, "playabilityStatus.reason") as string | null;
            throw new Error(reason || "No downloadable streams found");
        }

        const ext = bestMime.indexOf("webm") >= 0 ? "webm" : "mp4";
        const filename = amigo.sanitizeFilename(title) + "." + ext;

        amigo.logInfo("Selected: " + bestQuality + " (" + bestMime + ")");

        return {
            name: title,
            downloads: [{
                url: bestUrl,
                filename: filename,
                filesize: bestSize,
                chunks_supported: false,
                max_chunks: null,
                headers: null,
                cookies: null,
                wait_seconds: null,
                mirrors: [],
            }],
        };
    },

    checkOnline(url: string): "online" | "offline" | "unknown" {
        const videoId = extractVideoId(url);
        if (!videoId) return "unknown";

        const resp = amigo.httpGet(
            "https://www.youtube.com/oembed?url=https://www.youtube.com/watch?v=" + videoId + "&format=json"
        );

        if (resp.status === 200) return "online";
        if (resp.status === 404 || resp.status === 401) return "offline";
        return "unknown";
    },
} satisfies AmigoPlugin;
