import { describe, expect, it } from "vitest";
import { Browser } from "../../../src/browser/index.js";
import { createPluginContext } from "../../../src/context/index.js";
import { createMockHostApi, setHostApi } from "../../../src/host/index.js";
import type { HostHttpResponse } from "../../../src/host/index.js";
import { hls, selectBestVariant } from "../../../src/media/index.js";
import { definePlugin } from "../../../src/plugin/index.js";
import { formatInfo } from "../../../src/types/index.js";

// Scenario: extract best variant from an HLS master playlist.

const MASTER = `#EXTM3U
#EXT-X-VERSION:4
#EXT-X-STREAM-INF:BANDWIDTH=800000,RESOLUTION=640x360
low.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=2500000,RESOLUTION=1280x720
mid.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=5000000,RESOLUTION=1920x1080
high.m3u8
`;

describe("HLS extractor", () => {
  it("parses the master, selects best variant under user cap, returns FormatInfo", async () => {
    const controller = createMockHostApi({
      http: (): HostHttpResponse => ({
        status: 200,
        url: "https://cdn.test/stream/master.m3u8",
        redirectLocation: null,
        headers: { "Content-Type": "application/vnd.apple.mpegurl" },
        body: new TextEncoder().encode(MASTER),
      }),
    });
    setHostApi(controller.api);

    const plugin = definePlugin({
      id: "hls-demo",
      version: "1.0.0",
      match: [/cdn\.test\/stream\//],
      async extract(context) {
        const page = await context.browser.getPage(context.url);
        const master = hls.parseMaster(page.body(), context.url);
        const best = selectBestVariant(master.variants, { maxHeight: 720 });
        if (!best) {
          throw new Error("no variant");
        }
        return [
          formatInfo({
            url: best.url,
            manifestUrl: context.url,
            mediaType: "hls",
            width: best.resolution?.width ?? null,
            height: best.resolution?.height ?? null,
            bandwidth: best.bandwidth,
          }),
        ];
      },
    });

    const browser = new Browser({ hostApi: controller.api });
    const context = createPluginContext({
      url: "https://cdn.test/stream/master.m3u8",
      hostApi: controller.api,
      browser,
    });
    const formats = await plugin.extract!(context);
    expect(formats).toHaveLength(1);
    expect(formats[0]?.mediaType).toBe("hls");
    expect(formats[0]?.height).toBe(720);
    expect(formats[0]?.bandwidth).toBe(2_500_000);
  });
});
