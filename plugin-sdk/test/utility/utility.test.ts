import { describe, expect, it } from "vitest";
import {
  formatFilename,
  parseDate,
  parseDuration,
  parseSize,
} from "../../src/utility/index.js";

describe("parseSize", () => {
  it("parses SI and IEC units", () => {
    expect(parseSize("500")).toBe(500);
    expect(parseSize("1 KB")).toBe(1000);
    expect(parseSize("1 KiB")).toBe(1024);
    expect(parseSize("1.5 GB")).toBe(1_500_000_000);
    expect(parseSize("2 TiB")).toBe(2n === 2n ? 2 * 1024 ** 4 : 0);
  });

  it("strips thousands separators", () => {
    expect(parseSize("1,024 KB")).toBe(1_024_000);
  });

  it("returns null on garbage", () => {
    expect(parseSize("abc")).toBeNull();
    expect(parseSize("10 ZB")).toBeNull();
  });
});

describe("parseDuration", () => {
  it("parses clock notation", () => {
    expect(parseDuration("1:30:00")).toBe(5_400_000);
    expect(parseDuration("2:45")).toBe(165_000);
  });

  it("parses unit strings", () => {
    expect(parseDuration("2h 30m")).toBe(9_000_000);
    expect(parseDuration("45 seconds")).toBe(45_000);
    expect(parseDuration("1.5 min")).toBe(90_000);
  });

  it("returns null when unparseable", () => {
    expect(parseDuration("tomorrow")).toBeNull();
    expect(parseDuration("")).toBeNull();
  });
});

describe("parseDate", () => {
  it("understands 'X ago'", () => {
    const now = 1_700_000_000_000;
    const date = parseDate("2 hours ago", { now });
    expect(date?.getTime()).toBe(now - 2 * 3_600_000);
  });

  it("understands 'in X'", () => {
    const now = 1_700_000_000_000;
    const date = parseDate("in 5 minutes", { now });
    expect(date?.getTime()).toBe(now + 5 * 60_000);
  });

  it("handles yesterday/tomorrow", () => {
    const now = 1_700_000_000_000;
    expect(parseDate("yesterday", { now })?.getTime()).toBe(now - 86_400_000);
    expect(parseDate("tomorrow", { now })?.getTime()).toBe(now + 86_400_000);
  });

  it("parses ISO timestamps via Date.parse", () => {
    const date = parseDate("2024-01-02T03:04:05Z");
    expect(date?.toISOString()).toBe("2024-01-02T03:04:05.000Z");
  });
});

describe("formatFilename", () => {
  it("replaces Windows-illegal chars", () => {
    expect(formatFilename(`foo:bar/baz?.mkv`)).toBe("foo_bar_baz_.mkv");
  });

  it("collapses whitespace and trims trailing dots", () => {
    expect(formatFilename("  a   b.  ")).toBe("a b");
  });

  it("prefixes reserved Windows names", () => {
    expect(formatFilename("CON.txt")).toBe("_CON.txt");
  });

  it("normalises diacritics when requested", () => {
    expect(formatFilename("Café", { normaliseDiacritics: true })).toBe("Cafe");
  });

  it("truncates preserving extension", () => {
    const name = "a".repeat(300) + ".txt";
    const result = formatFilename(name, { maxLength: 50 });
    expect(result.endsWith(".txt")).toBe(true);
    expect(result.length).toBeLessThanOrEqual(50);
  });
});
