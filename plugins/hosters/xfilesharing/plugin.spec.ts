/// <reference path="../../types/amigo.d.ts" />

const plugin = module.exports;

test("has required metadata", () => {
    assertEqual(plugin.id, "xfilesharing");
    assertEqual(plugin.name, "XFileSharingPro (Generic)");
    assertNotNull(plugin.version);
    assertNotNull(plugin.urlPattern);
});

test("urlPattern matches known XFS domains", () => {
    const re = new RegExp(plugin.urlPattern);
    assert(re.test("https://ddownload.com/abc123"), "should match ddownload.com");
    assert(re.test("https://katfile.com/abc.html"), "should match katfile.com");
    assert(re.test("https://www.hexupload.net/file/test"), "should match www.hexupload.net");
    assert(!re.test("https://example.com/file.zip"), "should not match unknown domains");
});

test("has resolve function", () => {
    assertEqual(typeof plugin.resolve, "function");
});

test("has checkOnline function", () => {
    assertEqual(typeof plugin.checkOnline, "function");
});

test("size parsing: single capture group gets number + unit", () => {
    // Verify the regex pattern used for size extraction captures "number + unit"
    // so that the unit is not lost when regexMatch returns only the first group.
    const testCases = [
        { html: '<span>File size: 1.5 GB</span>', expectedBytes: Math.round(1.5 * 1024 * 1024 * 1024) },
        { html: '<span>Size: 500 MB</span>', expectedBytes: Math.round(500 * 1024 * 1024) },
        { html: '<span>100 KB file</span>', expectedBytes: Math.round(100 * 1024) },
        { html: '<span>2 TB storage</span>', expectedBytes: Math.round(2 * 1024 * 1024 * 1024 * 1024) },
    ];

    const sizePattern = "(\\d[\\d.]*\\s*(?:KB|MB|GB|TB))";
    const unitPattern = "(KB|MB|GB|TB)";
    const multipliers: Record<string, number> = {
        "KB": 1024,
        "MB": 1024 * 1024,
        "GB": 1024 * 1024 * 1024,
        "TB": 1024 * 1024 * 1024 * 1024,
    };

    for (const tc of testCases) {
        const sizeFullMatch = amigo.regexMatch(sizePattern, tc.html);
        assertNotNull(sizeFullMatch, "should find size in: " + tc.html);
        const num = parseFloat(sizeFullMatch!);
        const unitMatch = amigo.regexMatch(unitPattern, sizeFullMatch!);
        assertNotNull(unitMatch, "should find unit in: " + sizeFullMatch);
        const bytes = Math.round(num * multipliers[unitMatch!.toUpperCase()]);
        assertEqual(bytes, tc.expectedBytes, "bytes mismatch for: " + tc.html);
    }
});
