import { describe, expect, it } from "vitest";
import { Browser } from "../../../src/browser/index.js";
import { createPluginContext } from "../../../src/context/index.js";
import { createMockHostApi, setHostApi } from "../../../src/host/index.js";
import type { HostHttpResponse } from "../../../src/host/index.js";
import { defineDecrypter } from "../../../src/plugin/index.js";

// Scenario: a URL shortener that redirects (maybe twice) to the target URL.

function noBody(partial: Partial<HostHttpResponse> & { url: string }): HostHttpResponse {
  return {
    status: 200,
    redirectLocation: null,
    headers: {},
    body: new Uint8Array(),
    ...partial,
  };
}

describe("redirect shortener resolver", () => {
  it("follows the redirect chain and emits the final URL", async () => {
    const map: Record<string, HostHttpResponse> = {
      "https://short.test/abc": {
        status: 302,
        url: "https://short.test/abc",
        redirectLocation: "https://middle.test/hop",
        headers: {},
        body: new Uint8Array(),
      },
      "https://middle.test/hop": {
        status: 302,
        url: "https://middle.test/hop",
        redirectLocation: "https://target.test/final",
        headers: {},
        body: new Uint8Array(),
      },
      "https://target.test/final": noBody({ url: "https://target.test/final", status: 200 }),
    };
    const controller = createMockHostApi({
      http: (request) => map[request.url] ?? noBody({ url: request.url, status: 404 }),
    });
    setHostApi(controller.api);

    const plugin = defineDecrypter({
      id: "shortener",
      version: "1.0.0",
      match: [/short\.test\//],
      async decrypt(context) {
        const page = await context.browser.getPage(context.url);
        return [page.url];
      },
    });

    const browser = new Browser({ hostApi: controller.api });
    const context = createPluginContext({
      url: "https://short.test/abc",
      hostApi: controller.api,
      browser,
    });
    const links = await plugin.decrypt!(context);
    expect(links.map((l) => l.url)).toEqual(["https://target.test/final"]);
  });
});
