export type HttpMethod = "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS";

export type HeaderMap = Readonly<Record<string, string>>;

export interface HostHttpRequest {
  readonly method: HttpMethod;
  readonly url: string;
  readonly headers?: HeaderMap;
  readonly body?: string | Uint8Array;
  readonly followRedirects?: boolean;
  readonly maxRedirects?: number;
  readonly timeoutMilliseconds?: number;
  readonly signal?: AbortSignal;
}

export interface HostHttpResponse {
  readonly status: number;
  readonly url: string;
  readonly redirectLocation: string | null;
  readonly headers: HeaderMap;
  readonly body: Uint8Array;
}

export interface HostHtmlElement {
  readonly tag: string;
  readonly text: string;
  readonly html: string;
  readonly attributes: HeaderMap;
  readonly children: readonly HostHtmlElement[];
}

export interface HostHtmlDocument {
  readonly baseUrl: string | null;
  readonly root: HostHtmlElement;
  select(selector: string): readonly HostHtmlElement[];
  selectFirst(selector: string): HostHtmlElement | null;
}

export interface HostCryptoApi {
  aesCbcDecrypt(data: Uint8Array, key: Uint8Array, iv: Uint8Array): Uint8Array;
  aesCbcEncrypt(data: Uint8Array, key: Uint8Array, iv: Uint8Array): Uint8Array;
  md5(data: Uint8Array): Uint8Array;
  sha1(data: Uint8Array): Uint8Array;
  sha256(data: Uint8Array): Uint8Array;
  randomBytes(length: number): Uint8Array;
}

export interface HostUtilApi {
  base64Encode(data: Uint8Array): string;
  base64Decode(data: string): Uint8Array;
  hexEncode(data: Uint8Array): string;
  hexDecode(data: string): Uint8Array;
  textEncode(data: string): Uint8Array;
  textDecode(data: Uint8Array): string;
  urlEncode(data: string): string;
  urlDecode(data: string): string;
  now(): number;
  sleep(milliseconds: number, signal?: AbortSignal): Promise<void>;
}

export interface HostJavascriptEvalOptions {
  readonly timeoutMilliseconds?: number;
  readonly memoryLimitBytes?: number;
  readonly input?: unknown;
}

export interface HostJavascriptApi {
  eval<T = unknown>(code: string, options?: HostJavascriptEvalOptions): Promise<T>;
}

export type HostCaptchaKind =
  | "recaptcha_v2"
  | "recaptcha_v3"
  | "hcaptcha"
  | "turnstile"
  | "image"
  | "interactive";

export interface HostCaptchaRequest {
  readonly kind: HostCaptchaKind;
  readonly siteKey?: string;
  readonly pageUrl?: string;
  readonly action?: string;
  readonly imageUrl?: string;
  readonly prompt?: string;
  readonly mode?: "text" | "math";
  readonly invisible?: boolean;
  readonly signal?: AbortSignal;
}

export interface HostCaptchaResult {
  readonly token: string;
  readonly jobId?: string;
}

export interface HostCaptchaApi {
  solve(request: HostCaptchaRequest): Promise<HostCaptchaResult>;
  reportFailed?(jobId: string): void | Promise<void>;
}

export type HostPermission =
  | "javascript_eval"
  | "captcha"
  | "account"
  | "container"
  | "http_external";

export interface HostPermissionsApi {
  has(permission: HostPermission | string): boolean;
}

export type HostErrorCode =
  | "TimeoutError"
  | "AbortError"
  | "HttpError"
  | "PermissionDenied"
  | "BudgetExceeded"
  | "BodyTooLarge"
  | "ParseError"
  | "EvalError"
  | "Internal";

export interface HostErrorShape {
  readonly code: HostErrorCode;
  readonly message: string;
  readonly status?: number;
  readonly cause?: unknown;
}
