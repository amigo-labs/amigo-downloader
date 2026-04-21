import { describe, expect, it } from "vitest";
import { Browser } from "../../../src/browser/index.js";
import { createPluginContext } from "../../../src/context/index.js";
import { createMockHostApi, setHostApi } from "../../../src/host/index.js";
import type { HostHttpResponse } from "../../../src/host/index.js";
import { defineDecrypter } from "../../../src/plugin/index.js";

// Scenario: a paginated JSON API that lists file URLs page by page.

function encode(value: unknown): Uint8Array {
  return new TextEncoder().encode(JSON.stringify(value));
}

describe("paginated API crawler", () => {
  it("follows cursor-style pagination until the API stops", async () => {
    const controller = createMockHostApi({
      http: (request): HostHttpResponse => {
        const url = new URL(request.url);
        const cursor = url.searchParams.get("cursor") ?? "0";
        if (cursor === "0") {
          return {
            status: 200,
            url: request.url,
            redirectLocation: null,
            headers: { "Content-Type": "application/json" },
            body: encode({ items: ["https://a.test/1", "https://a.test/2"], next: "1" }),
          };
        }
        if (cursor === "1") {
          return {
            status: 200,
            url: request.url,
            redirectLocation: null,
            headers: { "Content-Type": "application/json" },
            body: encode({ items: ["https://a.test/3"], next: null }),
          };
        }
        return {
          status: 404,
          url: request.url,
          redirectLocation: null,
          headers: {},
          body: new Uint8Array(),
        };
      },
    });
    setHostApi(controller.api);

    const plugin = defineDecrypter({
      id: "api-crawler",
      version: "1.0.0",
      match: [/api\.example\.test\/folder\//],
      async decrypt(context) {
        const urls: string[] = [];
        let cursor: string | null = "0";
        while (cursor !== null) {
          const page = await context.browser.getPage(
            `https://api.example.test/list?cursor=${cursor}`,
          );
          const body = page.json<{ items: string[]; next: string | null }>();
          urls.push(...body.items);
          cursor = body.next;
        }
        return urls;
      },
    });

    const browser = new Browser({ hostApi: controller.api });
    const context = createPluginContext({
      url: "https://api.example.test/folder/42",
      hostApi: controller.api,
      browser,
    });
    const links = await plugin.decrypt!(context);
    expect(links.map((l) => l.url)).toEqual([
      "https://a.test/1",
      "https://a.test/2",
      "https://a.test/3",
    ]);
  });
});
