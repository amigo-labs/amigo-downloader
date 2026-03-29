/// <reference path="../types/amigo.d.ts" />

const plugin = module.exports;

test("has required metadata", () => {
    assertEqual(plugin.id, "youtube");
    assertEqual(plugin.name, "YouTube");
    assertNotNull(plugin.version);
    assertNotNull(plugin.urlPattern);
});

test("urlPattern matches youtube URLs", () => {
    const re = new RegExp(plugin.urlPattern);
    assert(re.test("https://www.youtube.com/watch?v=dQw4w9WgXcQ"), "watch URL");
    assert(re.test("https://youtube.com/shorts/abc12345678"), "shorts URL");
    assert(re.test("https://youtu.be/dQw4w9WgXcQ"), "short URL");
    assert(!re.test("https://example.com/video"), "should not match non-youtube");
});

test("has resolve function", () => {
    assertEqual(typeof plugin.resolve, "function");
});

test("has checkOnline function", () => {
    assertEqual(typeof plugin.checkOnline, "function");
});

// --- Integration tests (require network access to YouTube) ---
// These skip automatically when YouTube is unreachable (CI, firewalled envs).

function requireYouTube(): void {
    try {
        const resp = amigo.httpGet("https://www.youtube.com/oembed?url=https://www.youtube.com/watch?v=aqz-KE-bpKQ&format=json");
        if (resp.status !== 200) skip("YouTube not reachable (status " + resp.status + ")");
    } catch (e) {
        skip("YouTube not reachable: " + e);
    }
}

test("resolve returns valid DownloadPackage for known video", () => {
    requireYouTube();

    // Big Buck Bunny trailer — Creative Commons, stable URL
    const result = plugin.resolve("https://www.youtube.com/watch?v=aqz-KE-bpKQ");

    assertNotNull(result, "resolve should return a result");
    assertNotNull(result.name, "package should have a name");
    assert(result.name.length > 0, "name should not be empty");

    assertNotNull(result.downloads, "package should have downloads");
    assert(result.downloads.length > 0, "should have at least one download");

    const dl = result.downloads[0];
    assertNotNull(dl.url, "download should have a URL");
    assert(dl.url.indexOf("http") === 0, "URL should start with http");
    assertNotNull(dl.filename, "download should have a filename");
    assert(
        dl.filename.indexOf(".mp4") >= 0 || dl.filename.indexOf(".webm") >= 0,
        "filename should have video extension"
    );
});

test("resolve works with short URL format", () => {
    requireYouTube();

    const result = plugin.resolve("https://youtu.be/aqz-KE-bpKQ");

    assertNotNull(result, "resolve should return a result");
    assert(result.downloads.length > 0, "should have downloads");
    assert(result.downloads[0].url.indexOf("http") === 0, "URL should be valid");
});

test("resolve throws on invalid video ID", () => {
    requireYouTube();

    let threw = false;
    try {
        plugin.resolve("https://www.youtube.com/watch?v=XXXXXXXXXXX");
    } catch (e) {
        threw = true;
    }
    // Invalid/nonexistent video should throw (no streams or playability error)
    assert(threw, "resolve should throw for invalid video ID");
});

test("checkOnline returns online for known video", () => {
    requireYouTube();

    const status = plugin.checkOnline("https://www.youtube.com/watch?v=aqz-KE-bpKQ");
    assertEqual(status, "online", "Big Buck Bunny should be online");
});

test("checkOnline returns offline for nonexistent video", () => {
    requireYouTube();

    const status = plugin.checkOnline("https://www.youtube.com/watch?v=XXXXXXXXXXX");
    // Nonexistent video should be offline or unknown, but not online
    assert(status !== "online", "nonexistent video should not be online");
});
