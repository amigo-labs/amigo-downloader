// YouTube plugin for amigo-downloader
//
// Note: The native Rust implementation in crates/extractors/ handles YouTube
// downloads with the android_vr client + N-parameter challenge. This plugin
// serves as a JS-based fallback and reference implementation.

function pluginId() { return "youtube"; }
function pluginName() { return "YouTube"; }
function pluginVersion() { return "1.0.0"; }
function pluginDescription() { return "Download videos from YouTube"; }
function pluginAuthor() { return "amigo-labs"; }
function urlPattern() { return "https?://(www\\.)?(youtube\\.com/(watch|shorts|embed)|youtu\\.be/)"; }

function resolve(url) {
    var videoId = extractVideoId(url);
    if (!videoId) throw new Error("Could not extract video ID from URL");

    amigo.logInfo("Resolving YouTube video: " + videoId);

    var body = JSON.stringify({
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
                gl: "US"
            }
        }
    });

    var resp = JSON.parse(amigo.httpPost(
        "https://www.youtube.com/youtubei/v1/player?prettyPrint=false",
        body,
        "application/json"
    ));

    if (resp.status !== 200) {
        throw new Error("YouTube API returned status " + resp.status);
    }

    var data = JSON.parse(resp.body);
    var title = data.videoDetails && data.videoDetails.title;
    if (!title) throw new Error("Could not get video title");

    // Find best combined format (audio + video)
    var bestUrl = null;
    var bestQuality = "";
    var bestHeight = 0;
    var bestSize = null;
    var bestMime = "video/mp4";

    var formats = data.streamingData && data.streamingData.formats;
    if (formats) {
        for (var i = 0; i < formats.length; i++) {
            var fmt = formats[i];
            if (fmt.url && fmt.height && fmt.height > bestHeight) {
                bestUrl = fmt.url;
                bestHeight = fmt.height;
                bestQuality = fmt.qualityLabel || "unknown";
                bestSize = fmt.contentLength ? parseInt(fmt.contentLength) : null;
                bestMime = (fmt.mimeType || "video/mp4").split(";")[0];
            }
        }
    }

    // Fallback to adaptive formats (video-only)
    if (!bestUrl) {
        var adaptive = data.streamingData && data.streamingData.adaptiveFormats;
        if (adaptive) {
            for (var j = 0; j < adaptive.length; j++) {
                var afmt = adaptive[j];
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
        var reason = data.playabilityStatus && data.playabilityStatus.reason;
        throw new Error(reason || "No downloadable streams found");
    }

    var ext = bestMime.indexOf("webm") >= 0 ? "webm" : "mp4";
    var filename = title.replace(/[\/\\:*?"<>|]/g, "_") + "." + ext;

    amigo.logInfo("Selected: " + bestQuality + " (" + bestMime + ")");

    return {
        url: bestUrl,
        filename: filename,
        filesize: bestSize,
        chunks_supported: false,
        max_chunks: null,
        headers: null,
        cookies: null,
        wait_seconds: null,
        mirrors: [],
    };
}

function checkOnline(url) {
    var videoId = extractVideoId(url);
    if (!videoId) return "unknown";

    var resp = JSON.parse(amigo.httpGet(
        "https://www.youtube.com/oembed?url=https://www.youtube.com/watch?v=" + videoId + "&format=json"
    ));

    if (resp.status === 200) return "online";
    if (resp.status === 404 || resp.status === 401) return "offline";
    return "unknown";
}

function extractVideoId(url) {
    // youtube.com/watch?v=ID
    var m = amigo.regexMatch("[?&]v=([a-zA-Z0-9_-]{11})", url);
    if (m) return m;

    // youtu.be/ID
    m = amigo.regexMatch("youtu\\.be/([a-zA-Z0-9_-]{11})", url);
    if (m) return m;

    // /embed/ID or /shorts/ID
    m = amigo.regexMatch("/(embed|shorts|v)/([a-zA-Z0-9_-]{11})", url);
    if (m) return m;

    return null;
}
