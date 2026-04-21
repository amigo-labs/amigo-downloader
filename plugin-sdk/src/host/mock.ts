import {
  base64Decode,
  base64Encode,
  hexDecode,
  hexEncode,
} from "../extraction/encoding.js";
import type { HostApi } from "./api.js";
import type {
  HostCaptchaApi,
  HostCryptoApi,
  HostHtmlDocument,
  HostHttpRequest,
  HostHttpResponse,
  HostJavascriptApi,
  HostPermissionsApi,
  HostUtilApi,
} from "./types.js";

export type MockHttpDispatcher = (
  request: HostHttpRequest,
) => HostHttpResponse | Promise<HostHttpResponse> | Error;

export interface MockHostApiOptions {
  readonly http?: MockHttpDispatcher;
  readonly html?: HostApi["html"];
  readonly crypto?: Partial<HostCryptoApi>;
  readonly util?: Partial<HostUtilApi>;
  readonly javascript?: HostJavascriptApi;
  readonly captcha?: HostCaptchaApi;
  readonly permissions?: HostPermissionsApi | readonly string[];
}

export interface MockHostApiController {
  readonly api: HostApi;
  readonly requests: readonly HostHttpRequest[];
  reset(): void;
  setHttpDispatcher(dispatcher: MockHttpDispatcher): void;
}

function defaultUtil(): HostUtilApi {
  return {
    base64Encode,
    base64Decode,
    hexEncode: (data) => hexEncode(data),
    hexDecode,
    textEncode: (data) => new TextEncoder().encode(data),
    textDecode: (data) => new TextDecoder("utf-8").decode(data),
    urlEncode: (data) => encodeURIComponent(data),
    urlDecode: (data) => decodeURIComponent(data),
    now: () => Date.now(),
    sleep: (milliseconds, signal) =>
      new Promise<void>((resolve, reject) => {
        if (signal?.aborted) {
          reject(makeAbortError());
          return;
        }
        const timer = setTimeout(() => resolve(), milliseconds);
        signal?.addEventListener("abort", () => {
          clearTimeout(timer);
          reject(makeAbortError());
        });
      }),
  };
}

function makeAbortError(): Error {
  const error = new Error("Aborted");
  error.name = "AbortError";
  return error;
}

function defaultCrypto(): HostCryptoApi {
  const notConfigured = (name: string) => () => {
    throw new Error(`MockHostApi.crypto.${name} not configured`);
  };
  return {
    aesCbcDecrypt: notConfigured("aesCbcDecrypt"),
    aesCbcEncrypt: notConfigured("aesCbcEncrypt"),
    md5: notConfigured("md5"),
    sha1: notConfigured("sha1"),
    sha256: notConfigured("sha256"),
    randomBytes: (length) => {
      const bytes = new Uint8Array(length);
      crypto.getRandomValues(bytes);
      return bytes;
    },
  };
}

function defaultJavascript(): HostJavascriptApi {
  return {
    eval: () => {
      throw new Error("MockHostApi.javascript.eval not configured");
    },
  };
}

function defaultCaptcha(): HostCaptchaApi {
  return {
    solve: () => {
      throw new Error("MockHostApi.captcha.solve not configured");
    },
  };
}

function defaultHtml(): HostApi["html"] {
  return {
    parse: (): HostHtmlDocument => {
      throw new Error("MockHostApi.html.parse not configured");
    },
  };
}

export function createMockHostApi(options: MockHostApiOptions = {}): MockHostApiController {
  const recorded: HostHttpRequest[] = [];
  let dispatcher: MockHttpDispatcher | undefined = options.http;

  const util: HostUtilApi = { ...defaultUtil(), ...options.util };
  const cryptoApi: HostCryptoApi = { ...defaultCrypto(), ...options.crypto };
  const javascript = options.javascript ?? defaultJavascript();
  const captcha = options.captcha ?? defaultCaptcha();
  const html = options.html ?? defaultHtml();
  const permissions: HostPermissionsApi = (() => {
    if (!options.permissions) {
      return { has: () => true };
    }
    if (Array.isArray(options.permissions)) {
      const granted = new Set(options.permissions);
      return { has: (permission) => granted.has(permission) };
    }
    return options.permissions as HostPermissionsApi;
  })();

  const api: HostApi = {
    async http(request) {
      recorded.push(request);
      if (!dispatcher) {
        throw new Error("MockHostApi: no http dispatcher configured");
      }
      const result = await dispatcher(request);
      if (result instanceof Error) {
        throw result;
      }
      return result;
    },
    html,
    crypto: cryptoApi,
    util,
    javascript,
    captcha,
    permissions,
  };

  return {
    api,
    get requests() {
      return recorded;
    },
    reset() {
      recorded.length = 0;
    },
    setHttpDispatcher(next) {
      dispatcher = next;
    },
  };
}
