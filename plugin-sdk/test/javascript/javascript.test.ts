import { afterEach, describe, expect, it, vi } from "vitest";
import * as javascript from "../../src/javascript/index.js";
import { clearHostApi, createMockHostApi, setHostApi } from "../../src/host/index.js";

describe("javascript.run", () => {
  afterEach(() => clearHostApi());

  it("delegates to hostApi.javascript.eval when permission is granted", async () => {
    const evalFn = vi.fn(async () => 42);
    const host = createMockHostApi({
      javascript: { eval: evalFn },
    }).api;
    setHostApi(host);
    const result = await javascript.run<number>("return 42");
    expect(result).toBe(42);
    expect(evalFn).toHaveBeenCalledWith("return 42", {});
  });

  it("passes input and options through", async () => {
    const evalFn = vi.fn(async () => "ok");
    const host = createMockHostApi({
      javascript: { eval: evalFn },
    }).api;
    setHostApi(host);
    await javascript.run("code", { x: 1 }, { timeoutMilliseconds: 100, memoryLimitBytes: 1024 });
    expect(evalFn).toHaveBeenCalledWith(
      "code",
      expect.objectContaining({
        input: { x: 1 },
        timeoutMilliseconds: 100,
        memoryLimitBytes: 1024,
      }),
    );
  });

  it("throws PermissionDenied when permission missing", async () => {
    const host = createMockHostApi({ permissions: [] }).api;
    setHostApi(host);
    await expect(javascript.run("code")).rejects.toMatchObject({
      code: "PermissionDenied",
    });
  });

  it("wraps host errors into EvalError", async () => {
    const host = createMockHostApi({
      javascript: {
        eval: async () => {
          throw new Error("boom");
        },
      },
    }).api;
    setHostApi(host);
    await expect(javascript.run("code")).rejects.toMatchObject({
      code: "EvalError",
    });
  });
});

describe("javascript.unpackDeanEdwards", () => {
  afterEach(() => clearHostApi());

  it("rejects inputs that do not look like Dean-Edwards output", async () => {
    const host = createMockHostApi({
      javascript: { eval: async () => "" },
    }).api;
    setHostApi(host);
    await expect(javascript.unpackDeanEdwards("alert(1)")).rejects.toMatchObject({
      code: "ParseError",
    });
  });
});
