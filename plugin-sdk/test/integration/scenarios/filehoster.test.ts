import { describe, expect, it, vi } from "vitest";
import { Browser } from "../../../src/browser/index.js";
import * as captcha from "../../../src/captcha/index.js";
import { createPluginContext } from "../../../src/context/index.js";
import { createMockHostApi, setHostApi } from "../../../src/host/index.js";
import type {
  HostCaptchaApi,
  HostHtmlDocument,
  HostHtmlElement,
  HostHttpResponse,
} from "../../../src/host/index.js";
import { definePlugin } from "../../../src/plugin/index.js";
import { formatInfo } from "../../../src/types/index.js";

// Scenario: a typical filehoster that shows a landing page,
// requires the user to wait, then solve a reCaptcha to reveal the final link.

function encode(text: string): Uint8Array {
  return new TextEncoder().encode(text);
}

function element(html: string, attrs: Record<string, string> = {}): HostHtmlElement {
  return { tag: "div", text: "", html, attributes: attrs, children: [] };
}

function document(forms: HostHtmlElement[], recaptcha: HostHtmlElement | null): HostHtmlDocument {
  return {
    baseUrl: null,
    root: element(""),
    select: (selector) => {
      if (selector === "form") {
        return forms;
      }
      if (selector === ".g-recaptcha" || selector === "[data-sitekey]") {
        return recaptcha ? [recaptcha] : [];
      }
      return [];
    },
    selectFirst: (selector) => {
      if (selector === ".g-recaptcha" || selector === "[data-sitekey]") {
        return recaptcha;
      }
      if (selector === "form") {
        return forms[0] ?? null;
      }
      return null;
    },
  };
}

describe("filehoster flow with countdown and reCaptcha", () => {
  it("walks the three-step process and emits a FormatInfo", async () => {
    const landingHtml = `<html>
      <form action="/continue" method="post"><input type="hidden" name="id" value="xyz"></form>
    </html>`;
    const captchaPageHtml = `<html>
      <div class="g-recaptcha" data-sitekey="SITEKEY"></div>
      <form action="/final" method="post"><input type="hidden" name="token" value="CAPTCHA"></form>
    </html>`;
    const finalHtml = `<html>Download:
      <a id="dl" href="https://cdn.example.test/files/xyz.zip">link</a>
    </html>`;

    const pages: Record<string, string> = {
      "https://files.example.test/file/xyz": landingHtml,
      "https://files.example.test/continue": captchaPageHtml,
      "https://files.example.test/final": finalHtml,
    };

    const hostCaptcha: HostCaptchaApi = {
      solve: vi.fn(async () => ({ token: "captcha-token-ok", jobId: "job" })),
    };

    const controller = createMockHostApi({
      http: (request) => {
        const body = pages[request.url];
        if (!body) {
          return {
            status: 404,
            url: request.url,
            redirectLocation: null,
            headers: {},
            body: new Uint8Array(),
          } satisfies HostHttpResponse;
        }
        return {
          status: 200,
          url: request.url,
          redirectLocation: null,
          headers: {},
          body: encode(body),
        };
      },
      html: {
        parse: (source) => {
          if (source.includes("g-recaptcha")) {
            const recaptcha = element("", {
              class: "g-recaptcha",
              "data-sitekey": "SITEKEY",
            });
            const form = element(
              `<form action="/final" method="post"><input type="hidden" name="token" value="CAPTCHA"></form>`,
            );
            return document([form], recaptcha);
          }
          if (source.includes("/continue")) {
            const form = element(
              `<form action="/continue" method="post"><input type="hidden" name="id" value="xyz"></form>`,
            );
            return document([form], null);
          }
          return document([], null);
        },
      },
      captcha: hostCaptcha,
    });
    setHostApi(controller.api);

    const plugin = definePlugin({
      id: "filehoster-demo",
      version: "1.0.0",
      match: [/files\.example\.test\/file\//],
      async extract(context) {
        const landing = await context.browser.getPage(context.url);
        const continueForm = landing.getForm();
        if (!continueForm) {
          throw new Error("continue form missing");
        }
        await context.wait(10);
        const captchaPage = await continueForm.submit();
        const challenge = await captcha.recaptchaV2(captchaPage);
        const submit = captchaPage.getForm();
        if (!submit) {
          throw new Error("submit form missing");
        }
        const finalPage = await submit.submit({
          "g-recaptcha-response": challenge.token,
        });
        const href = finalPage.regex(/href="([^"]+\.zip)"/).getMatch(1);
        if (!href) {
          throw new Error("download URL not found");
        }
        return [formatInfo({ url: href, filename: "xyz.zip" })];
      },
    });

    const browser = new Browser({ hostApi: controller.api });
    const context = createPluginContext({
      url: "https://files.example.test/file/xyz",
      hostApi: controller.api,
      browser,
    });

    const formats = await plugin.extract!(context);
    expect(formats).toHaveLength(1);
    expect(formats[0]?.url).toBe("https://cdn.example.test/files/xyz.zip");
    expect(formats[0]?.filename).toBe("xyz.zip");
    expect(hostCaptcha.solve).toHaveBeenCalled();
  });
});
