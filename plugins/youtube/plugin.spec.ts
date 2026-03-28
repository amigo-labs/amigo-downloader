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
