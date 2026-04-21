import type { HostApi } from "./api.js";
import type {
  HostCaptchaApi,
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
  readonly captcha?: HostCaptchaApi;
}

export interface MockHostApiController {
  readonly api: HostApi;
  readonly requests: readonly HostHttpRequest[];
  reset(): void;
  setHttpDispatcher(dispatcher: MockHttpDispatcher): void;
}

function bytesToBase64(data: Uint8Array): string {
  let binary = "";
  for (let index = 0; index < data.length; index += 1) {
    binary += String.fromCharCode(data[index]!);
  }
  return btoa(binary);
}

function base64ToBytes(data: string): Uint8Array {
  const binary = atob(data);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

function bytesToHex(data: Uint8Array): string {
  let hex = "";
  for (let index = 0; index < data.length; index += 1) {
    hex += data[index]!.toString(16).padStart(2, "0");
  }
  return hex;
}

function hexToBytes(data: string): Uint8Array {
  if (data.length % 2 !== 0) {
    throw new Error("hexDecode: odd-length input");
  }
  const bytes = new Uint8Array(data.length / 2);
  for (let index = 0; index < bytes.length; index += 1) {
    const byte = Number.parseInt(data.slice(index * 2, index * 2 + 2), 16);
    if (Number.isNaN(byte)) {
      throw new Error(`hexDecode: invalid byte at index ${index}`);
    }
    bytes[index] = byte;
  }
  return bytes;
}

function defaultUtil(): HostUtilApi {
  return {
    base64Encode: bytesToBase64,
    base64Decode: base64ToBytes,
    hexEncode: bytesToHex,
    hexDecode: hexToBytes,
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
