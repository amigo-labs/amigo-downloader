import { Buffer } from "node:buffer";
import { webcrypto } from "node:crypto";
import type { HostApi } from "./api.js";
import type {
  HostCryptoApi,
  HostHtmlDocument,
  HostHttpRequest,
  HostHttpResponse,
  HostJavascriptApi,
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
}

export interface MockHostApiController {
  readonly api: HostApi;
  readonly requests: readonly HostHttpRequest[];
  reset(): void;
  setHttpDispatcher(dispatcher: MockHttpDispatcher): void;
}

function defaultUtil(): HostUtilApi {
  return {
    base64Encode: (data) => Buffer.from(data).toString("base64"),
    base64Decode: (data) => new Uint8Array(Buffer.from(data, "base64")),
    hexEncode: (data) => Buffer.from(data).toString("hex"),
    hexDecode: (data) => new Uint8Array(Buffer.from(data, "hex")),
    textEncode: (data) => new TextEncoder().encode(data),
    textDecode: (data) => new TextDecoder("utf-8").decode(data),
    urlEncode: (data) => encodeURIComponent(data),
    urlDecode: (data) => decodeURIComponent(data),
    now: () => Date.now(),
    sleep: (milliseconds, signal) =>
      new Promise<void>((resolve, reject) => {
        if (signal?.aborted) {
          reject(new DOMException("Aborted", "AbortError"));
          return;
        }
        const timer = setTimeout(() => resolve(), milliseconds);
        signal?.addEventListener("abort", () => {
          clearTimeout(timer);
          reject(new DOMException("Aborted", "AbortError"));
        });
      }),
  };
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
      webcrypto.getRandomValues(bytes);
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
  const crypto: HostCryptoApi = { ...defaultCrypto(), ...options.crypto };
  const javascript = options.javascript ?? defaultJavascript();
  const html = options.html ?? defaultHtml();

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
    crypto,
    util,
    javascript,
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
