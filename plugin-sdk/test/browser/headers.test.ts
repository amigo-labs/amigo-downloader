import { describe, expect, it } from "vitest";
import { Headers } from "../../src/browser/headers.js";

describe("Headers", () => {
  it("is case-insensitive on lookup but preserves set-casing", () => {
    const headers = new Headers();
    headers.set("Content-Type", "application/json");
    expect(headers.get("content-type")).toBe("application/json");
    expect(headers.get("CONTENT-TYPE")).toBe("application/json");
    expect(headers.toRecord()).toEqual({ "Content-Type": "application/json" });
  });

  it("overwrites previous casing when re-set", () => {
    const headers = new Headers();
    headers.set("X-Token", "a");
    headers.set("x-token", "b");
    expect(headers.get("X-Token")).toBe("b");
    expect(headers.toRecord()).toEqual({ "x-token": "b" });
  });

  it("returns null for missing keys, not undefined or throw", () => {
    const headers = new Headers();
    expect(headers.get("missing")).toBeNull();
  });

  it("clone produces independent instance", () => {
    const original = new Headers({ "X-A": "1" });
    const copy = original.clone();
    copy.set("X-B", "2");
    expect(original.has("X-B")).toBe(false);
    expect(copy.has("X-A")).toBe(true);
  });

  it("iteration yields preserved casing", () => {
    const headers = new Headers({ "User-Agent": "test/1.0" });
    const pairs = Array.from(headers);
    expect(pairs).toEqual([["User-Agent", "test/1.0"]]);
  });
});
