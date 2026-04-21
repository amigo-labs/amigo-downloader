import { describe, expect, it } from "vitest";
import { createPluginContext } from "../../src/context/index.js";
import {
  defineDecrypter,
  definePlugin,
  matchesAny,
} from "../../src/plugin/index.js";
import { createMockHostApi } from "../../src/host/index.js";
import { fileInfo, formatInfo } from "../../src/types/index.js";

describe("URL matching", () => {
  it("regex match", () => {
    expect(matchesAny([/example\.test\/video/], "https://example.test/video/1")).toBe(true);
    expect(matchesAny([/example\.test\/video/], "https://other.test/")).toBe(false);
  });

  it("glob-style string match", () => {
    expect(matchesAny(["https://example.test/*"], "https://example.test/anywhere")).toBe(true);
    expect(matchesAny(["https://example.test/*"], "https://other.test/")).toBe(false);
  });
});

describe("definePlugin", () => {
  it("builds a Plugin with matches() and manifest()", () => {
    const plugin = definePlugin({
      id: "example-hoster",
      version: "1.2.3",
      match: [/example\.test\//],
      async extract() {
        return [formatInfo({ url: "https://cdn.example.test/a.mp4" })];
      },
    });

    expect(plugin.kind).toBe("hoster");
    expect(plugin.matches("https://example.test/a")).toBe(true);
    expect(plugin.manifest().id).toBe("example-hoster");
  });

  it("runs extract() against a PluginContext", async () => {
    const controller = createMockHostApi();
    const plugin = definePlugin({
      id: "p",
      version: "1.0.0",
      match: [/./],
      async extract(context) {
        return [formatInfo({ url: `${context.url}/direct` })];
      },
    });
    const context = createPluginContext({ url: "https://example.test/a", hostApi: controller.api });
    const result = await plugin.extract!(context);
    expect(result[0]?.url).toBe("https://example.test/a/direct");
  });

  it("checkAvailable is plumbed when provided", async () => {
    const controller = createMockHostApi();
    const plugin = definePlugin({
      id: "p",
      version: "1.0.0",
      match: [/./],
      async checkAvailable() {
        return fileInfo({ filename: "x", size: 42, availability: "online" });
      },
      async extract() {
        return [];
      },
    });
    const context = createPluginContext({ url: "https://example.test/a", hostApi: controller.api });
    expect(plugin.checkAvailable).toBeDefined();
    const info = await plugin.checkAvailable!(context);
    expect(info.size).toBe(42);
  });

  it("rejects definitions with empty id/match", () => {
    expect(() =>
      definePlugin({
        id: "",
        version: "1.0.0",
        match: [/./],
        extract: async () => [],
      }),
    ).toThrow(/missing id/);
    expect(() =>
      definePlugin({
        id: "x",
        version: "1.0.0",
        match: [],
        extract: async () => [],
      }),
    ).toThrow(/no match patterns/);
  });
});

describe("defineDecrypter", () => {
  it("normalises string results into DownloadLink shape", async () => {
    const controller = createMockHostApi();
    const plugin = defineDecrypter({
      id: "folder",
      version: "0.1.0",
      match: [/./],
      async decrypt() {
        return ["https://a.test/1", "https://a.test/2"];
      },
    });
    const context = createPluginContext({ url: "https://a.test/folder", hostApi: controller.api });
    const links = await plugin.decrypt!(context);
    expect(links).toHaveLength(2);
    expect(links[0]?.url).toBe("https://a.test/1");
    expect(links[0]?.filename).toBeNull();
  });
});

describe("PluginConfig", () => {
  it("distinguishes stored null from absent key", async () => {
    const { pluginConfig } = await import("../../src/types/plugin-config.js");
    const config = pluginConfig({ present: null });
    expect(config.get("present", "default")).toBeNull();
    expect(config.get("missing", "default")).toBe("default");
    expect(config.has("present")).toBe(true);
    expect(config.has("missing")).toBe(false);
  });
});

describe("PluginContext helpers", () => {
  it("link() accepts strings", () => {
    const controller = createMockHostApi();
    const context = createPluginContext({ url: "https://a.test/", hostApi: controller.api });
    expect(context.link("https://cdn.test/x").url).toBe("https://cdn.test/x");
  });

  it("wait() resolves after host sleep", async () => {
    const controller = createMockHostApi();
    const context = createPluginContext({ url: "https://a.test/", hostApi: controller.api });
    const start = Date.now();
    await context.wait(10);
    expect(Date.now() - start).toBeGreaterThanOrEqual(5);
  });

  it("log()/progress() call listeners", () => {
    const logs: string[] = [];
    const progress: number[] = [];
    const controller = createMockHostApi();
    const context = createPluginContext({
      url: "https://a.test/",
      hostApi: controller.api,
      onLog: (record) => logs.push(`${record.level}:${record.message}`),
      onProgress: (record) => progress.push(record.current),
    });
    context.log("info", "hello");
    context.progress(5, 10, "halfway");
    expect(logs).toEqual(["info:hello"]);
    expect(progress).toEqual([5]);
  });
});
