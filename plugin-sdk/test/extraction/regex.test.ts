import { describe, expect, it } from "vitest";
import { regex, sourceContains } from "../../src/extraction/regex.js";

describe("regex", () => {
  it("returns null on no match without throwing", () => {
    const result = regex("abc", "xyz");
    expect(result.matches()).toBe(false);
    expect(result.getMatch(0)).toBeNull();
    expect(result.getMatches()).toEqual([]);
    expect(result.getColumn(1)).toEqual([]);
  });

  it("extracts first match with getMatch(group)", () => {
    const result = regex(
      '<a href="https://a.test/1">1</a><a href="https://a.test/2">2</a>',
      /href="([^"]+)"/,
    );
    expect(result.getMatch(1)).toBe("https://a.test/1");
  });

  it("getColumn pulls a specific group across matches", () => {
    const result = regex("a=1;b=2;c=3", /([a-z])=(\d)/);
    expect(result.getColumn(1)).toEqual(["a", "b", "c"]);
    expect(result.getColumn(2)).toEqual(["1", "2", "3"]);
  });

  it("accepts string pattern and injects global flag", () => {
    const result = regex("aaa", "a");
    expect(result.getMatches()).toHaveLength(3);
  });
});

describe("sourceContains", () => {
  it("matches substring and regex", () => {
    expect(sourceContains("hello world", "world")).toBe(true);
    expect(sourceContains("foo 42 bar", /\d+/)).toBe(true);
    expect(sourceContains("abc", "xyz")).toBe(false);
  });
});
