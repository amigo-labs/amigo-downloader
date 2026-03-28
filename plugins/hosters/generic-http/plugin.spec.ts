/// <reference path="../types/amigo.d.ts" />

const plugin = module.exports;

test("has required metadata", () => {
    assertEqual(plugin.id, "generic-http");
    assertEqual(plugin.name, "Generic HTTP");
    assertNotNull(plugin.version);
    assertNotNull(plugin.urlPattern);
});

test("urlPattern matches any HTTP URL", () => {
    const re = new RegExp(plugin.urlPattern);
    assert(re.test("https://example.com/file.zip"), "should match https");
    assert(re.test("http://example.com/file.zip"), "should match http");
});

test("has resolve function", () => {
    assertEqual(typeof plugin.resolve, "function");
});

test("has checkOnline function", () => {
    assertEqual(typeof plugin.checkOnline, "function");
});
