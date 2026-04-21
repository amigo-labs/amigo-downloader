import { describe, expect, it } from "vitest";
import * as errors from "../../src/errors/index.js";
import {
  ERROR_CODES,
  PluginError,
  isErrorCode,
  isPluginError,
  toPluginError,
} from "../../src/errors/index.js";

describe("PluginError", () => {
  it("carries code and message", () => {
    const error = new PluginError("FileNotFound", { message: "not there" });
    expect(error.code).toBe("FileNotFound");
    expect(error.message).toBe("not there");
    expect(error.name).toBe("PluginError");
  });

  it("carries retryAfterMilliseconds and details", () => {
    const error = new PluginError("Retry", {
      retryAfterMilliseconds: 5000,
      details: { attempt: 2 },
    });
    expect(error.retryAfterMilliseconds).toBe(5000);
    expect(error.details).toEqual({ attempt: 2 });
  });

  it("serialize produces a plain object", () => {
    const error = new PluginError("HttpError", {
      message: "upstream 500",
      details: { status: 500 },
    });
    const serialized = error.serialize();
    expect(serialized.code).toBe("HttpError");
    expect(serialized.details).toEqual({ status: 500 });
    expect(serialized.retryAfterMilliseconds).toBeNull();
  });
});

describe("toPluginError", () => {
  it("returns the same instance for PluginError", () => {
    const error = new PluginError("Fatal");
    expect(toPluginError(error)).toBe(error);
  });

  it("wraps native Error in PluginDefect preserving stack", () => {
    const inner = new Error("boom");
    const error = toPluginError(inner);
    expect(error.code).toBe("PluginDefect");
    expect(error.message).toBe("boom");
    expect((error as Error).cause).toBe(inner);
  });

  it("wraps unknown throws (strings, objects) in PluginDefect", () => {
    expect(toPluginError("oops").code).toBe("PluginDefect");
    expect(toPluginError(42).code).toBe("PluginDefect");
  });
});

describe("error factories throw PluginError with correct code", () => {
  it.each(ERROR_CODES.filter((code) => code !== "HttpError"))("%s", (code) => {
    const factoryName = code.charAt(0).toLowerCase() + code.slice(1);
    const factory = (errors as unknown as Record<string, () => never>)[factoryName];
    if (typeof factory !== "function") {
      throw new Error(`missing factory for ${code} (expected ${factoryName})`);
    }
    try {
      factory();
      throw new Error("factory did not throw");
    } catch (error) {
      expect(isPluginError(error)).toBe(true);
      if (isPluginError(error)) {
        expect(error.code).toBe(code);
      }
    }
  });

  it("httpError records status in details", () => {
    try {
      errors.httpError(418);
    } catch (error) {
      if (!isPluginError(error)) {
        throw error;
      }
      expect(error.code).toBe("HttpError");
      expect(error.details).toMatchObject({ status: 418 });
    }
  });
});

describe("isErrorCode", () => {
  it("validates known codes", () => {
    expect(isErrorCode("FileNotFound")).toBe(true);
    expect(isErrorCode("Unknown")).toBe(false);
    expect(isErrorCode(42)).toBe(false);
  });
});
