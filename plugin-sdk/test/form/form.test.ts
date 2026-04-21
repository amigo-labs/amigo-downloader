import { describe, expect, it } from "vitest";
import { Browser } from "../../src/browser/index.js";
import { Form, buildFormFromHtml } from "../../src/form/index.js";
import { createMockHostApi } from "../../src/host/index.js";
import type {
  HostHtmlDocument,
  HostHtmlElement,
  HostHttpRequest,
  HostHttpResponse,
} from "../../src/host/index.js";

function emptyElement(html: string, tag = "form"): HostHtmlElement {
  return { tag, text: "", html, attributes: {}, children: [] };
}

function htmlDocument(elements: readonly HostHtmlElement[]): HostHtmlDocument {
  return {
    baseUrl: null,
    root: emptyElement("", "root"),
    select: (selector) => (selector === "form" ? elements : []),
    selectFirst: (selector) => (selector === "form" ? elements[0] ?? null : null),
  };
}

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

describe("buildFormFromHtml", () => {
  const mock = createMockHostApi();
  const browser = new Browser({ hostApi: mock.api });

  it("parses action, method, and input values", () => {
    const form = buildFormFromHtml(
      mock.api,
      browser,
      "https://example.test/page",
      `<form action="/login" method="post">
        <input name="user" value="alice">
        <input name="pw" type="password" value="secret">
        <input name="token" type="hidden" value="abc">
      </form>`,
    );
    expect(form.method).toBe("POST");
    expect(form.action).toBe("https://example.test/login");
    expect(form.toRecord()).toEqual({ user: "alice", pw: "secret", token: "abc" });
  });

  it("skips submit/button inputs, unchecked checkboxes, and unnamed fields", () => {
    const form = buildFormFromHtml(
      mock.api,
      browser,
      "https://example.test/",
      `<form>
        <input name="a" value="1">
        <input name="b" type="checkbox" value="on">
        <input name="c" type="checkbox" checked value="on">
        <input type="submit" name="go" value="Send">
        <input value="nameless">
      </form>`,
    );
    expect(form.toRecord()).toEqual({ a: "1", c: "on" });
  });

  it("defaults to GET and resolves action against base URL when empty", () => {
    const form = buildFormFromHtml(
      mock.api,
      browser,
      "https://example.test/dir/page",
      `<form><input name="q" value="x"></form>`,
    );
    expect(form.method).toBe("GET");
    expect(form.action).toBe("https://example.test/dir/page");
  });
});

describe("Form submission", () => {
  it("POSTs url-encoded body with merged overrides", async () => {
    const seen: HostHttpRequest[] = [];
    const controller = createMockHostApi({
      http: (request) => {
        seen.push(request);
        return ok({ url: request.url, body: new Uint8Array() });
      },
    });
    const browser = new Browser({ hostApi: controller.api });
    const form = buildFormFromHtml(
      controller.api,
      browser,
      "https://example.test/",
      `<form action="/login" method="post"><input name="user" value="alice"><input name="pw" value=""></form>`,
    );
    await form.submit({ pw: "secret" });
    expect(seen[0]?.method).toBe("POST");
    expect(seen[0]?.headers?.["Content-Type"]).toBe("application/x-www-form-urlencoded");
    const body = typeof seen[0]?.body === "string" ? seen[0]?.body : "";
    expect(body).toBe("user=alice&pw=secret");
  });

  it("GET form appends query params to URL", async () => {
    const seen: HostHttpRequest[] = [];
    const controller = createMockHostApi({
      http: (request) => {
        seen.push(request);
        return ok({ url: request.url, body: new Uint8Array() });
      },
    });
    const browser = new Browser({ hostApi: controller.api });
    const form = buildFormFromHtml(
      controller.api,
      browser,
      "https://example.test/",
      `<form action="/search"><input name="q" value="hi"></form>`,
    );
    await form.submit({ q: "bye", page: "2" });
    expect(seen[0]?.method).toBe("GET");
    expect(seen[0]?.url).toBe("https://example.test/search?q=bye&page=2");
  });
});

describe("Page.getForm", () => {
  it("extracts form from parsed document", async () => {
    const formElement = emptyElement(
      `<form action="/submit" method="post"><input name="a" value="1"></form>`,
    );
    const controller = createMockHostApi({
      http: (request) => ok({ url: request.url, body: encode("<html></html>") }),
      html: {
        parse: () => htmlDocument([formElement]),
      },
    });
    const browser = new Browser({ hostApi: controller.api });
    const page = await browser.getPage("https://example.test/");
    const form = page.getForm();
    expect(form).toBeInstanceOf(Form);
    expect(form?.method).toBe("POST");
    expect(form?.toRecord()).toEqual({ a: "1" });
  });

  it("getForms returns all forms", async () => {
    const forms = [
      emptyElement(`<form id="a"><input name="x" value="1"></form>`),
      emptyElement(`<form id="b"><input name="y" value="2"></form>`),
    ];
    const controller = createMockHostApi({
      http: (request) => ok({ url: request.url, body: encode("<html></html>") }),
      html: { parse: () => htmlDocument(forms) },
    });
    const browser = new Browser({ hostApi: controller.api });
    const page = await browser.getPage("https://example.test/");
    const all = page.getForms();
    expect(all).toHaveLength(2);
    expect(all[0]?.get("x")).toBe("1");
    expect(all[1]?.get("y")).toBe("2");
  });
});
