import type { HostApi } from "../host/api.js";
import { getHostApi } from "../host/injection.js";
import type {
  HeaderMap,
  HostHttpRequest,
  HostHttpResponse,
  HttpMethod,
} from "../host/types.js";
import { CookieJar } from "./cookies.js";
import { Headers } from "./headers.js";
import { Page, type PageSnapshot } from "./page.js";
import { matchRegex, sourceContains, type RegexResult } from "./regex-result.js";

export interface BrowserOptions {
  readonly hostApi?: HostApi;
  readonly cookies?: CookieJar;
  readonly defaultHeaders?: Readonly<Record<string, string>>;
  readonly followRedirects?: boolean;
  readonly maxRedirects?: number;
  readonly timeoutMilliseconds?: number;
}

export interface BrowserRequestOptions {
  readonly method?: HttpMethod;
  readonly url: string;
  readonly headers?: Readonly<Record<string, string>>;
  readonly body?: string | Uint8Array;
  readonly followRedirects?: boolean;
  readonly maxRedirects?: number;
  readonly timeoutMilliseconds?: number;
  readonly signal?: AbortSignal;
}

const DEFAULT_MAX_REDIRECTS = 10;
const DEFAULT_TIMEOUT_MS = 30_000;

function extractSetCookies(headers: HeaderMap): string[] {
  for (const [name, value] of Object.entries(headers)) {
    if (name.toLowerCase() === "set-cookie") {
      return value.split(/\r?\n/).filter((line) => line.trim().length > 0);
    }
  }
  return [];
}

function resolveRedirect(currentUrl: string, location: string): string {
  return new URL(location, currentUrl).toString();
}

function isRedirect(status: number): boolean {
  return status === 301 || status === 302 || status === 303 || status === 307 || status === 308;
}

export class Browser {
  private readonly hostApi: HostApi;
  private readonly defaultHeaders: Headers;
  private readonly cookies: CookieJar;
  private followRedirectsDefault: boolean;
  private maxRedirectsDefault: number;
  private timeoutDefault: number;
  private currentPage: Page | null = null;
  private explicitReferer: string | null | undefined = undefined;

  constructor(options: BrowserOptions = {}) {
    this.hostApi = options.hostApi ?? getHostApi();
    this.defaultHeaders = new Headers(options.defaultHeaders);
    this.cookies = options.cookies ?? new CookieJar();
    this.followRedirectsDefault = options.followRedirects ?? true;
    this.maxRedirectsDefault = options.maxRedirects ?? DEFAULT_MAX_REDIRECTS;
    this.timeoutDefault = options.timeoutMilliseconds ?? DEFAULT_TIMEOUT_MS;
  }

  get page(): Page | null {
    return this.currentPage;
  }

  getUrl(): string | null {
    return this.currentPage?.url ?? null;
  }

  getStatus(): number | null {
    return this.currentPage?.status ?? null;
  }

  getHeader(name: string): string | null {
    return this.currentPage?.headers.get(name) ?? null;
  }

  body(): string {
    if (this.currentPage === null) {
      throw new Error("Browser has no current response yet");
    }
    return this.currentPage.body();
  }

  bodyBytes(): Uint8Array {
    if (this.currentPage === null) {
      throw new Error("Browser has no current response yet");
    }
    return this.currentPage.bodyBytes();
  }

  json<T = unknown>(): T {
    if (this.currentPage === null) {
      throw new Error("Browser has no current response yet");
    }
    return this.currentPage.json<T>();
  }

  containsHTML(pattern: string | RegExp): boolean {
    return this.currentPage ? sourceContains(this.currentPage.body(), pattern) : false;
  }

  regex(pattern: string | RegExp, flags?: string): RegexResult {
    if (this.currentPage === null) {
      return matchRegex("", pattern, flags);
    }
    return matchRegex(this.currentPage.body(), pattern, flags);
  }

  setHeader(name: string, value: string): void {
    this.defaultHeaders.set(name, value);
  }

  setHeaders(headers: Readonly<Record<string, string>>): void {
    this.defaultHeaders.setAll(headers);
  }

  clearHeader(name: string): void {
    this.defaultHeaders.delete(name);
  }

  setUserAgent(userAgent: string): void {
    this.defaultHeaders.set("User-Agent", userAgent);
  }

  setReferer(referer: string | null): void {
    this.explicitReferer = referer;
    if (referer === null) {
      this.defaultHeaders.delete("Referer");
    } else {
      this.defaultHeaders.set("Referer", referer);
    }
  }

  setFollowRedirects(enabled: boolean): void {
    this.followRedirectsDefault = enabled;
  }

  setMaxRedirects(count: number): void {
    this.maxRedirectsDefault = count;
  }

  setTimeout(milliseconds: number): void {
    this.timeoutDefault = milliseconds;
  }

  get cookieJar(): CookieJar {
    return this.cookies;
  }

  async getPage(url: string, options?: Omit<BrowserRequestOptions, "url" | "method">): Promise<Page> {
    return this.request({ ...options, method: "GET", url });
  }

  async postPage(
    url: string,
    form: Readonly<Record<string, string>>,
    options?: Omit<BrowserRequestOptions, "url" | "method" | "body">,
  ): Promise<Page> {
    const body = new URLSearchParams(form).toString();
    const headers = {
      "Content-Type": "application/x-www-form-urlencoded",
      ...(options?.headers ?? {}),
    };
    return this.request({ ...options, method: "POST", url, body, headers });
  }

  async postPageRaw(
    url: string,
    body: string | Uint8Array,
    contentType: string,
    options?: Omit<BrowserRequestOptions, "url" | "method" | "body">,
  ): Promise<Page> {
    const headers = { "Content-Type": contentType, ...(options?.headers ?? {}) };
    return this.request({ ...options, method: "POST", url, body, headers });
  }

  async headPage(url: string, options?: Omit<BrowserRequestOptions, "url" | "method">): Promise<Page> {
    return this.request({ ...options, method: "HEAD", url });
  }

  async request(options: BrowserRequestOptions): Promise<Page> {
    const method = options.method ?? "GET";
    const followRedirects = options.followRedirects ?? this.followRedirectsDefault;
    const maxRedirects = options.maxRedirects ?? this.maxRedirectsDefault;
    const timeout = options.timeoutMilliseconds ?? this.timeoutDefault;

    let currentUrl = new URL(options.url).toString();
    let currentMethod: HttpMethod = method;
    let currentBody: string | Uint8Array | undefined = options.body;
    let redirectsLeft = maxRedirects;
    const visited = new Set<string>();

    for (;;) {
      if (visited.has(currentUrl)) {
        throw new Error(`Redirect loop detected at ${currentUrl}`);
      }
      visited.add(currentUrl);

      const requestHeaders = this.buildHeaders(currentUrl, options.headers);
      const request: HostHttpRequest = {
        method: currentMethod,
        url: currentUrl,
        headers: requestHeaders,
        ...(currentBody !== undefined ? { body: currentBody } : {}),
        followRedirects: false,
        timeoutMilliseconds: timeout,
        ...(options.signal ? { signal: options.signal } : {}),
      };

      const response = await this.hostApi.http(request);
      this.storeSetCookies(currentUrl, response.headers);

      if (
        followRedirects &&
        redirectsLeft > 0 &&
        isRedirect(response.status) &&
        response.redirectLocation !== null
      ) {
        const nextUrl = resolveRedirect(currentUrl, response.redirectLocation);
        const nextMethod: HttpMethod = response.status === 303 ? "GET" : currentMethod;
        if (response.status === 303 || response.status === 301 || response.status === 302) {
          currentBody = nextMethod === "GET" || nextMethod === "HEAD" ? undefined : currentBody;
          currentMethod = nextMethod === "POST" ? "GET" : nextMethod;
        } else {
          currentMethod = nextMethod;
        }
        this.explicitReferer = currentUrl;
        this.defaultHeaders.set("Referer", currentUrl);
        currentUrl = nextUrl;
        redirectsLeft -= 1;
        continue;
      }

      const snapshot: PageSnapshot = {
        url: currentUrl,
        status: response.status,
        redirectLocation: response.redirectLocation,
        headers: new Headers(response.headers),
        bodyBytes: response.body,
      };
      const page = new Page(this.hostApi, snapshot);
      this.currentPage = page;
      this.explicitReferer = currentUrl;
      this.defaultHeaders.set("Referer", currentUrl);
      return page;
    }
  }

  clone(): Browser {
    const clone = new Browser({
      hostApi: this.hostApi,
      cookies: this.cookies,
      defaultHeaders: this.defaultHeaders.toRecord(),
      followRedirects: this.followRedirectsDefault,
      maxRedirects: this.maxRedirectsDefault,
      timeoutMilliseconds: this.timeoutDefault,
    });
    return clone;
  }

  private buildHeaders(
    currentUrl: string,
    overrides: Readonly<Record<string, string>> | undefined,
  ): HeaderMap {
    const merged = new Headers(this.defaultHeaders.toRecord());
    if (overrides) {
      merged.setAll(overrides);
    }
    const cookieHeader = this.cookies.get(currentUrl);
    if (cookieHeader.length > 0) {
      merged.set("Cookie", cookieHeader);
    }
    return merged.toRecord();
  }

  private storeSetCookies(currentUrl: string, headers: HeaderMap): void {
    const setCookies = extractSetCookies(headers);
    if (setCookies.length === 0) {
      return;
    }
    this.cookies.setAll(currentUrl, setCookies);
  }
}
