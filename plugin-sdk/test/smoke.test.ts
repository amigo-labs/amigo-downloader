import { describe, expect, it } from "vitest";
import * as sdk from "../src/index.js";

describe("plugin-sdk smoke", () => {
  it("exports SDK_VERSION", () => {
    expect(typeof sdk.SDK_VERSION).toBe("string");
  });

  it("exposes all planned module namespaces", () => {
    const expected = [
      "host",
      "browser",
      "extraction",
      "form",
      "errors",
      "captcha",
      "plugin",
      "context",
      "account",
      "media",
      "container",
      "javascript",
      "types",
    ] as const;
    for (const name of expected) {
      expect(sdk).toHaveProperty(name);
      expect(typeof (sdk as Record<string, unknown>)[name]).toBe("object");
    }
  });
});
