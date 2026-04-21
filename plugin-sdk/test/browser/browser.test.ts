import { beforeEach, describe, expect, it } from "vitest";
import { Browser } from "../../src/browser/index.js";
import { createMockHostApi } from "../../src/host/index.js";
import type { HostHttpRequest, HostHttpResponse } from "../../src/host/index.js";

function encode(body: string): Uint8Array {
  return new TextEncoder().encode(body);
}

function ok(partial: Partial<HostHttpResponse> & { url: string }): HostHttpResponse {
  return {
    status: 200,
    redirectLocation: null,
    headers: {},
    body: new Uint8Array(),
    ...partial,
  };
}

function redirect(from: string, to: string, status = 302): HostHttpResponse {
  return {
    status,
    url: from,
    redirectLocation: to,
    headers: {},
    body: new Uint8Array(),
  };
}

describe("Browser basic GET", () => {
  it("fetches a page and exposes body/status/headers", async () => {
    const controller = createMockHostApi({
      http: (request) =>
        ok({
          url: request.url,
          status: 200,
          headers: { "Content-Type": "text/plain" },
          body: encode("hello"),
        }),
    });
    const browser = new Browser({ hostApi: controller.api });
    const page = await browser.getPage("https://example.test/");
    expect(page.status).toBe(200);
    expect(page.body()).toBe("hello");
    expect(page.headers.get("content-type")).toBe("text/plain");
    expect(browser.getUrl()).toBe("https://example.test/");
  });

  it("adds Cookie header from jar on subsequent requests", async () => {
    const seen: HostHttpRequest[] = [];
    const controller = createMockHostApi({
      http: (request) => {
        seen.push(request);
        if (request.url.endsWith("/set")) {
          return ok({
            url: request.url,
            headers: { "Set-Cookie": "session=abc" },
            body: new Uint8Array(),
          });
        }
        return ok({ url: request.url, body: new Uint8Array() });
      },
    });
    const browser = new Browser({ hostApi: controller.api });
    await browser.getPage("https://example.test/set");
    await browser.getPage("https://example.test/again");
    expect(seen[1]?.headers?.["Cookie"]).toBe("session=abc");
  });

  it("sets Referer automatically on follow-up requests", async () => {
    const seen: HostHttpRequest[] = [];
    const controller = createMockHostApi({
      http: (request) => {
        seen.push(request);
        return ok({ url: request.url, body: new Uint8Array() });
      },
    });
    const browser = new Browser({ hostApi: controller.api });
    await browser.getPage("https://example.test/a");
    await browser.getPage("https://example.test/b");
    expect(seen[0]?.headers?.["Referer"]).toBeUndefined();
    expect(seen[1]?.headers?.["Referer"]).toBe("https://example.test/a");
  });

  it("setReferer(null) suppresses referer header", async () => {
    const seen: HostHttpRequest[] = [];
    const controller = createMockHostApi({
      http: (request) => {
        seen.push(request);
        return ok({ url: request.url, body: new Uint8Array() });
      },
    });
    const browser = new Browser({ hostApi: controller.api });
    await browser.getPage("https://example.test/a");
    browser.setReferer(null);
    await browser.getPage("https://example.test/b");
    expect(seen[1]?.headers?.["Referer"]).toBeUndefined();
  });
});

describe("Browser redirects", () => {
  it("follows 302 and ends at final URL", async () => {
    const responses: HostHttpResponse[] = [
      redirect("https://example.test/a", "https://example.test/b"),
      ok({ url: "https://example.test/b", body: encode("end") }),
    ];
    let index = 0;
    const controller = createMockHostApi({
      http: () => responses[index++]!,
    });
    const browser = new Browser({ hostApi: controller.api });
    const page = await browser.getPage("https://example.test/a");
    expect(page.url).toBe("https://example.test/b");
    expect(page.body()).toBe("end");
  });

  it("does not follow when setFollowRedirects(false)", async () => {
    const controller = createMockHostApi({
      http: () => redirect("https://example.test/a", "https://example.test/b"),
    });
    const browser = new Browser({ hostApi: controller.api, followRedirects: false });
    const page = await browser.getPage("https://example.test/a");
    expect(page.status).toBe(302);
    expect(page.redirectLocation).toBe("https://example.test/b");
  });

  it("detects redirect loops", async () => {
    const controller = createMockHostApi({
      http: (request) =>
        redirect(
          request.url,
          request.url.endsWith("/a") ? "https://example.test/b" : "https://example.test/a",
        ),
    });
    const browser = new Browser({ hostApi: controller.api, maxRedirects: 10 });
    await expect(browser.getPage("https://example.test/a")).rejects.toThrow(/Redirect loop/);
  });
});

describe("Browser body helpers", () => {
  let browser: Browser;
  beforeEach(async () => {
    const controller = createMockHostApi({
      http: () =>
        ok({
          url: "https://example.test/",
          body: encode('<div>foo</div>{"token":"abc"}'),
        }),
    });
    browser = new Browser({ hostApi: controller.api });
    await browser.getPage("https://example.test/");
  });

  it("containsHTML matches strings and regex", () => {
    expect(browser.containsHTML("foo")).toBe(true);
    expect(browser.containsHTML(/<div>.*<\/div>/)).toBe(true);
    expect(browser.containsHTML("missing")).toBe(false);
  });

  it("regex returns a RegexResult with groups", () => {
    const result = browser.regex(/"token":"([^"]+)"/);
    expect(result.matches()).toBe(true);
    expect(result.getMatch(1)).toBe("abc");
  });
});

describe("Browser clone", () => {
  it("shares cookies and copies default headers", async () => {
    const controller = createMockHostApi({
      http: (request) => ok({ url: request.url, body: new Uint8Array() }),
    });
    const original = new Browser({ hostApi: controller.api });
    original.setUserAgent("amigo-test/1.0");
    original.cookieJar.set("https://example.test/", "sid=1");

    const clone = original.clone();
    expect(clone.cookieJar).toBe(original.cookieJar);

    clone.setHeader("X-Only-Clone", "yes");
    expect(original.getHeader("X-Only-Clone")).toBeNull();
  });
});

describe("Browser with explicit Cookie header", () => {
  it("jar value takes precedence over per-request header", async () => {
    const seen: HostHttpRequest[] = [];
    const controller = createMockHostApi({
      http: (request) => {
        seen.push(request);
        return ok({ url: request.url, body: new Uint8Array() });
      },
    });
    const browser = new Browser({ hostApi: controller.api });
    browser.cookieJar.set("https://example.test/", "a=1");
    await browser.getPage("https://example.test/", { headers: { Cookie: "b=2" } });
    expect(seen[0]?.headers?.["Cookie"]).toBe("a=1");
  });
});
