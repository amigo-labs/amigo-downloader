import { describe, expect, it } from "vitest";
import {
  extract,
  getArray,
  getBoolean,
  getNumber,
  getObject,
  getString,
  parse,
  walk,
} from "../../src/extraction/json.js";

describe("json parse/walk", () => {
  it("parses", () => {
    expect(parse<{ a: number }>("{\"a\":1}").a).toBe(1);
  });

  it("walks nested paths", () => {
    const value = { a: { b: { c: 42 } } };
    expect(walk(value, "a/b/c")).toBe(42);
  });

  it("returns null for missing intermediates", () => {
    expect(walk({ a: null }, "a/b/c")).toBeNull();
    expect(walk({ a: {} }, "a/b/c")).toBeNull();
  });

  it("indexes into arrays with numeric segments", () => {
    expect(walk({ xs: [10, 20, 30] }, "xs/1")).toBe(20);
    expect(walk({ xs: [10] }, "xs/5")).toBeNull();
  });
});

describe("typed json getters", () => {
  const data = { s: "x", n: 5, b: true, arr: [1, 2], obj: { k: "v" } };
  it("returns typed values or null", () => {
    expect(getString(data, "s")).toBe("x");
    expect(getString(data, "n")).toBeNull();
    expect(getNumber(data, "n")).toBe(5);
    expect(getBoolean(data, "b")).toBe(true);
    expect(getArray<number>(data, "arr")).toEqual([1, 2]);
    expect(getObject(data, "obj")).toEqual({ k: "v" });
    expect(getObject(data, "arr")).toBeNull();
  });
});

describe("json.extract", () => {
  it("pulls values out of HTML with different quote styles", () => {
    expect(extract('"token":"abc"', "token")).toBe("abc");
    expect(extract("'token': 'abc'", "token")).toBe("abc");
    expect(extract("token: \"abc\"", "token")).toBe("abc");
  });

  it("returns null when key not present", () => {
    expect(extract('"other":"x"', "token")).toBeNull();
  });

  it("decodes JSON escape sequences in the extracted value", () => {
    expect(extract('"name":"a\\u0042c"', "name")).toBe("aBc");
  });

  it("handles numeric and boolean values", () => {
    expect(extract('"count": 42', "count")).toBe("42");
    expect(extract('"flag": true', "flag")).toBe("true");
  });
});
