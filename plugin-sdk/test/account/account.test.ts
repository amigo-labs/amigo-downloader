import { describe, expect, it } from "vitest";
import { accountStatus, session } from "../../src/account/index.js";
import { createPluginContext } from "../../src/context/index.js";
import { createMockHostApi } from "../../src/host/index.js";
import { definePlugin } from "../../src/plugin/index.js";
import { formatInfo } from "../../src/types/index.js";

describe("session()/accountStatus() factories", () => {
  it("fill defaults", () => {
    const snapshot = session({ headers: { Authorization: "Bearer x" } });
    expect(snapshot.cookies).toEqual([]);
    expect(snapshot.headers).toEqual({ Authorization: "Bearer x" });
    expect(snapshot.metadata).toEqual({});
    expect(snapshot.createdAt).toBeGreaterThan(0);

    const status = accountStatus({ validity: "valid", premium: true });
    expect(status.validity).toBe("valid");
    expect(status.premium).toBe(true);
    expect(status.expiresAt).toBeNull();
  });
});

describe("definePlugin with account config", () => {
  it("threads AccountConfig onto the Plugin", async () => {
    const controller = createMockHostApi();
    const plugin = definePlugin({
      id: "premium-host",
      version: "1.0.0",
      match: [/premium\.test\//],
      account: {
        async login() {
          return session({ metadata: { token: "t" } });
        },
        async check() {
          return accountStatus({ validity: "valid", premium: true });
        },
      },
      async extract() {
        return [formatInfo({ url: "https://cdn.premium.test/x" })];
      },
    });
    expect(plugin.account).not.toBeNull();

    const context = createPluginContext({
      url: "https://premium.test/item",
      hostApi: controller.api,
    });
    const sessionResult = await plugin.account!.login(context, {
      username: "u",
      password: "p",
      extra: {},
    });
    expect(sessionResult.metadata).toEqual({ token: "t" });

    const status = await plugin.account!.check(context, sessionResult);
    expect(status.premium).toBe(true);
  });

  it("decrypter plugins have null account", () => {
    const plugin = definePlugin({
      id: "p",
      version: "1.0.0",
      match: [/./],
      async extract() {
        return [];
      },
    });
    expect(plugin.account).toBeNull();
  });
});

describe("PluginContext.account", () => {
  it("carries full AccountContext when provided", () => {
    const controller = createMockHostApi();
    const context = createPluginContext({
      url: "https://example.test/",
      hostApi: controller.api,
      account: {
        id: "acct-1",
        status: accountStatus({ validity: "valid", premium: true }),
        session: session(),
        credentials: null,
      },
    });
    expect(context.account?.id).toBe("acct-1");
    expect(context.account?.status.premium).toBe(true);
  });
});
