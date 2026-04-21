import { afterEach, describe, expect, it, vi } from "vitest";
import { Browser } from "../../src/browser/index.js";
import * as captcha from "../../src/captcha/index.js";
import { PluginError } from "../../src/errors/index.js";
import { clearHostApi, createMockHostApi, setHostApi } from "../../src/host/index.js";
import type {
  HostCaptchaApi,
  HostCaptchaRequest,
  HostHtmlDocument,
  HostHtmlElement,
  HostHttpResponse,
} from "../../src/host/index.js";

function ok(body: Uint8Array = new Uint8Array()): HostHttpResponse {
  return {
    status: 200,
    url: "https://example.test/",
    redirectLocation: null,
    headers: {},
    body,
  };
}

function docWithRecaptcha(siteKey: string): HostHtmlDocument {
  const recaptcha: HostHtmlElement = {
    tag: "div",
    text: "",
    html: "",
    attributes: { class: "g-recaptcha", "data-sitekey": siteKey },
    children: [],
  };
  return {
    baseUrl: null,
    root: { tag: "root", text: "", html: "", attributes: {}, children: [recaptcha] },
    select: (selector) => {
      if (selector === ".g-recaptcha" || selector === "[data-sitekey]") {
        return [recaptcha];
      }
      return [];
    },
    selectFirst: (selector) => {
      if (selector === ".g-recaptcha" || selector === "[data-sitekey]") {
        return recaptcha;
      }
      return null;
    },
  };
}

function makeHostCaptcha(token: string): HostCaptchaApi {
  return {
    solve: vi.fn(async (_request: HostCaptchaRequest) => ({ token, jobId: "job-1" })),
    reportFailed: vi.fn(async () => {}),
  };
}

describe("captcha.recaptchaV2", () => {
  afterEach(() => clearHostApi());

  it("auto-detects siteKey from page and returns token", async () => {
    const hostCaptcha = makeHostCaptcha("tok-1");
    const controller = createMockHostApi({
      http: () => ok(new TextEncoder().encode("<html>page</html>")),
      html: { parse: () => docWithRecaptcha("SITEKEY_X") },
      captcha: hostCaptcha,
    });
    setHostApi(controller.api);
    const browser = new Browser({ hostApi: controller.api });
    const page = await browser.getPage("https://example.test/");
    const result = await captcha.recaptchaV2(page);
    expect(result.token).toBe("tok-1");
    expect(hostCaptcha.solve).toHaveBeenCalledWith(
      expect.objectContaining({
        kind: "recaptcha_v2",
        siteKey: "SITEKEY_X",
        pageUrl: "https://example.test/",
      }),
    );
  });

  it("throws CaptchaFailed when no site key is found and none is passed", async () => {
    const controller = createMockHostApi({
      http: () => ok(new TextEncoder().encode("<html/>")),
      html: {
        parse: () => ({
          baseUrl: null,
          root: { tag: "root", text: "", html: "", attributes: {}, children: [] },
          select: () => [],
          selectFirst: () => null,
        }),
      },
      captcha: makeHostCaptcha("x"),
    });
    setHostApi(controller.api);
    const browser = new Browser({ hostApi: controller.api });
    const page = await browser.getPage("https://example.test/");
    await expect(captcha.recaptchaV2(page)).rejects.toMatchObject({
      code: "CaptchaFailed",
    });
  });
});

describe("captcha.detect", () => {
  afterEach(() => clearHostApi());

  it("returns first matching widget", async () => {
    const controller = createMockHostApi({
      http: () => ok(new TextEncoder().encode("<html/>")),
      html: { parse: () => docWithRecaptcha("KEY") },
      captcha: makeHostCaptcha("x"),
    });
    setHostApi(controller.api);
    const browser = new Browser({ hostApi: controller.api });
    const page = await browser.getPage("https://example.test/");
    expect(captcha.detect(page)).toEqual({ kind: "recaptcha_v2", siteKey: "KEY" });
  });
});

describe("CaptchaResult.reportFailed", () => {
  afterEach(() => clearHostApi());

  it("calls host.captcha.reportFailed with jobId", async () => {
    const hostCaptcha = makeHostCaptcha("tok-2");
    const controller = createMockHostApi({
      http: () => ok(new TextEncoder().encode("<html/>")),
      html: { parse: () => docWithRecaptcha("KEY") },
      captcha: hostCaptcha,
    });
    setHostApi(controller.api);
    const browser = new Browser({ hostApi: controller.api });
    const page = await browser.getPage("https://example.test/");
    const result = await captcha.recaptchaV2(page);
    await result.reportFailed();
    expect(hostCaptcha.reportFailed).toHaveBeenCalledWith("job-1");
  });
});

describe("captcha.image", () => {
  afterEach(() => clearHostApi());

  it("posts image URL request to host", async () => {
    const hostCaptcha = makeHostCaptcha("abcd");
    const controller = createMockHostApi({ captcha: hostCaptcha });
    setHostApi(controller.api);
    const result = await captcha.image("https://example.test/captcha.png", { mode: "text" });
    expect(result.token).toBe("abcd");
    expect(hostCaptcha.solve).toHaveBeenCalledWith(
      expect.objectContaining({
        kind: "image",
        imageUrl: "https://example.test/captcha.png",
        mode: "text",
      }),
    );
  });
});

describe("captcha throws PluginError when host missing", () => {
  it("surfaces error code", () => {
    const error = new PluginError("CaptchaFailed");
    expect(error.code).toBe("CaptchaFailed");
  });
});
