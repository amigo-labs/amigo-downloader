import { beforeEach, describe, expect, it } from "vitest";
import {
  clearHostApi,
  createMockHostApi,
  getHostApi,
  hasHostApi,
  setHostApi,
} from "../../src/host/index.js";

describe("host-api injection", () => {
  beforeEach(() => {
    clearHostApi();
  });

  it("throws when no api is installed", () => {
    expect(hasHostApi()).toBe(false);
    expect(() => getHostApi()).toThrow(/HostApi not initialized/);
  });

  it("returns the installed api", () => {
    const controller = createMockHostApi();
    setHostApi(controller.api);
    expect(hasHostApi()).toBe(true);
    expect(getHostApi()).toBe(controller.api);
  });

  it("clearHostApi resets installation", () => {
    setHostApi(createMockHostApi().api);
    clearHostApi();
    expect(hasHostApi()).toBe(false);
  });
});
