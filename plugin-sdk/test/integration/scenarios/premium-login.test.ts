import { describe, expect, it } from "vitest";
import { Browser } from "../../../src/browser/index.js";
import { accountStatus, session } from "../../../src/account/index.js";
import { createPluginContext } from "../../../src/context/index.js";
import { createMockHostApi, setHostApi } from "../../../src/host/index.js";
import type { HostHttpResponse } from "../../../src/host/index.js";
import { definePlugin } from "../../../src/plugin/index.js";
import { formatInfo } from "../../../src/types/index.js";

// Scenario: premium hoster with login, session, and authenticated file lookup.

function encodeJson(value: unknown): Uint8Array {
  return new TextEncoder().encode(JSON.stringify(value));
}

describe("premium hoster with account login", () => {
  it("logs in, receives token, and uses it on file request", async () => {
    const controller = createMockHostApi({
      http: (request): HostHttpResponse => {
        if (request.url === "https://premium.test/login" && request.method === "POST") {
          const body = typeof request.body === "string" ? request.body : "";
          if (body === "username=alice&password=s3cret") {
            return {
              status: 200,
              url: request.url,
              redirectLocation: null,
              headers: { "Content-Type": "application/json" },
              body: encodeJson({ token: "AUTH-XYZ" }),
            };
          }
          return {
            status: 401,
            url: request.url,
            redirectLocation: null,
            headers: {},
            body: new Uint8Array(),
          };
        }
        if (
          request.url.startsWith("https://premium.test/file/") &&
          request.headers?.["Authorization"] === "Bearer AUTH-XYZ"
        ) {
          return {
            status: 200,
            url: request.url,
            redirectLocation: null,
            headers: { "Content-Type": "application/json" },
            body: encodeJson({ url: "https://cdn.premium.test/dl/42.bin", size: 1024 }),
          };
        }
        return {
          status: 403,
          url: request.url,
          redirectLocation: null,
          headers: {},
          body: new Uint8Array(),
        };
      },
    });
    setHostApi(controller.api);

    const plugin = definePlugin({
      id: "premium-demo",
      version: "1.0.0",
      match: [/premium\.test\/file\//],
      account: {
        async login(context, credentials) {
          const page = await context.browser.postPage("https://premium.test/login", {
            username: credentials.username,
            password: credentials.password,
          });
          const data = page.json<{ token: string }>();
          return session({
            headers: { Authorization: `Bearer ${data.token}` },
            metadata: { token: data.token },
          });
        },
        async check() {
          return accountStatus({ validity: "valid", premium: true });
        },
      },
      async extract(context) {
        if (!context.account) {
          throw new Error("no account");
        }
        const authHeader = (context.account.session.headers as Record<string, string>)["Authorization"] ?? "";
        const page = await context.browser.getPage(context.url, {
          headers: { Authorization: authHeader },
        });
        const data = page.json<{ url: string; size: number }>();
        return [formatInfo({ url: data.url, size: data.size })];
      },
    });

    const browser = new Browser({ hostApi: controller.api });
    const loginContext = createPluginContext({
      url: "https://premium.test/login",
      hostApi: controller.api,
      browser,
    });
    const sessionResult = await plugin.account!.login(loginContext, {
      username: "alice",
      password: "s3cret",
      extra: {},
    });
    expect(sessionResult.metadata["token"]).toBe("AUTH-XYZ");

    const fetchBrowser = new Browser({ hostApi: controller.api });
    const fetchContext = createPluginContext({
      url: "https://premium.test/file/42",
      hostApi: controller.api,
      browser: fetchBrowser,
      account: {
        id: "acc",
        session: sessionResult,
        status: accountStatus({ validity: "valid", premium: true }),
        credentials: null,
      },
    });
    const formats = await plugin.extract!(fetchContext);
    expect(formats[0]?.url).toBe("https://cdn.premium.test/dl/42.bin");
    expect(formats[0]?.size).toBe(1024);
  });
});
