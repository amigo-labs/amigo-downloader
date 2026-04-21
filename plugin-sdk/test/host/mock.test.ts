import { describe, expect, it } from "vitest";
import { createMockHostApi } from "../../src/host/index.js";
import type { HostHttpResponse } from "../../src/host/index.js";

function response(partial: Partial<HostHttpResponse> = {}): HostHttpResponse {
  return {
    status: 200,
    url: "https://example.test/",
    redirectLocation: null,
    headers: {},
    body: new Uint8Array(),
    ...partial,
  };
}

describe("MockHostApi http dispatcher", () => {
  it("returns the configured response and records the request", async () => {
    const controller = createMockHostApi({
      http: () => response({ status: 201, body: new TextEncoder().encode("ok") }),
    });

    const result = await controller.api.http({ method: "GET", url: "https://example.test/a" });

    expect(result.status).toBe(201);
    expect(new TextDecoder().decode(result.body)).toBe("ok");
    expect(controller.requests).toHaveLength(1);
    expect(controller.requests[0]?.url).toBe("https://example.test/a");
  });

  it("throws when the dispatcher returns an Error instance", async () => {
    const boom = new Error("upstream failed");
    const controller = createMockHostApi({ http: () => boom });

    await expect(
      controller.api.http({ method: "GET", url: "https://example.test/fail" }),
    ).rejects.toBe(boom);
    expect(controller.requests).toHaveLength(1);
  });

  it("throws when no dispatcher is configured", async () => {
    const controller = createMockHostApi();
    await expect(
      controller.api.http({ method: "GET", url: "https://example.test/" }),
    ).rejects.toThrow(/no http dispatcher/);
  });

  it("setHttpDispatcher swaps behaviour", async () => {
    const controller = createMockHostApi({ http: () => response({ status: 200 }) });
    controller.setHttpDispatcher(() => response({ status: 418 }));
    const result = await controller.api.http({ method: "GET", url: "https://example.test/" });
    expect(result.status).toBe(418);
  });

  it("reset clears request log", async () => {
    const controller = createMockHostApi({ http: () => response() });
    await controller.api.http({ method: "GET", url: "https://example.test/a" });
    controller.reset();
    expect(controller.requests).toHaveLength(0);
  });
});

describe("MockHostApi util defaults", () => {
  it("base64 roundtrips", () => {
    const { util } = createMockHostApi().api;
    const encoded = util.base64Encode(util.textEncode("hello"));
    expect(util.textDecode(util.base64Decode(encoded))).toBe("hello");
  });

  it("sleep rejects on abort", async () => {
    const { util } = createMockHostApi().api;
    const controller = new AbortController();
    const promise = util.sleep(10_000, controller.signal);
    controller.abort();
    await expect(promise).rejects.toThrow(/Aborted/);
  });
});
