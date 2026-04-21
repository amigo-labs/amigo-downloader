import { describe, expect, it } from "vitest";
import { CookieJar, parseSetCookie } from "../../src/browser/cookies.js";

describe("parseSetCookie", () => {
  it("parses a simple name=value cookie", () => {
    const cookie = parseSetCookie("session=abc", "https://example.test/page");
    expect(cookie?.name).toBe("session");
    expect(cookie?.value).toBe("abc");
    expect(cookie?.domain).toBe("example.test");
    expect(cookie?.hostOnly).toBe(true);
  });

  it("records Domain attribute and drops host-only flag", () => {
    const cookie = parseSetCookie(
      "sid=1; Domain=example.test; Path=/",
      "https://foo.example.test/page",
    );
    expect(cookie?.domain).toBe("example.test");
    expect(cookie?.hostOnly).toBe(false);
    expect(cookie?.path).toBe("/");
  });

  it("parses Secure and HttpOnly", () => {
    const cookie = parseSetCookie(
      "tok=x; Secure; HttpOnly",
      "https://example.test/",
    );
    expect(cookie?.secure).toBe(true);
    expect(cookie?.httpOnly).toBe(true);
  });

  it("honours Max-Age over Expires", () => {
    const cookie = parseSetCookie(
      "t=1; Max-Age=60; Expires=Thu, 01 Jan 1970 00:00:00 GMT",
      "https://example.test/",
    );
    expect(cookie?.expiresAt).toBeGreaterThan(Date.now());
  });
});

describe("CookieJar", () => {
  it("returns cookies applicable to the request host", () => {
    const jar = new CookieJar();
    jar.set("https://example.test/path", "a=1");
    expect(jar.get("https://example.test/other")).toBe("a=1");
  });

  it("host-only cookies do not leak to subdomains", () => {
    const jar = new CookieJar();
    jar.set("https://example.test/", "only=1");
    expect(jar.get("https://foo.example.test/")).toBe("");
  });

  it("domain cookies leak down to subdomains", () => {
    const jar = new CookieJar();
    jar.set("https://example.test/", "sid=1; Domain=example.test");
    expect(jar.get("https://foo.example.test/")).toBe("sid=1");
  });

  it("secure cookies are not sent over http", () => {
    const jar = new CookieJar();
    jar.set("https://example.test/", "t=1; Secure");
    expect(jar.get("http://example.test/")).toBe("");
  });

  it("later cookies with same name/domain/path overwrite earlier", () => {
    const jar = new CookieJar();
    jar.set("https://example.test/", "a=1");
    jar.set("https://example.test/", "a=2");
    expect(jar.get("https://example.test/")).toBe("a=2");
  });

  it("expired cookies are rejected", () => {
    const jar = new CookieJar();
    jar.set(
      "https://example.test/",
      "a=1; Expires=Thu, 01 Jan 1970 00:00:00 GMT",
    );
    expect(jar.size).toBe(0);
  });

  it("clearHost removes only matching domain", () => {
    const jar = new CookieJar();
    jar.set("https://example.test/", "a=1");
    jar.set("https://other.test/", "b=2");
    jar.clearHost("example.test");
    expect(jar.get("https://example.test/")).toBe("");
    expect(jar.get("https://other.test/")).toBe("b=2");
  });

  it("export/import is lossless", () => {
    const jar = new CookieJar();
    jar.set("https://example.test/", "a=1; Domain=example.test");
    const exported = jar.export();
    const restored = new CookieJar();
    restored.import(exported);
    expect(restored.get("https://foo.example.test/")).toBe("a=1");
  });

  it("path matching respects prefix rule", () => {
    const jar = new CookieJar();
    jar.set("https://example.test/area/", "a=1; Path=/area");
    expect(jar.get("https://example.test/area/page")).toBe("a=1");
    expect(jar.get("https://example.test/other")).toBe("");
  });
});
