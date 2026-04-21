import type { HostApi } from "../host/api.js";
import type { HostHtmlDocument } from "../host/types.js";
import { Element } from "./element.js";
import { Headers } from "./headers.js";
import { matchRegex, sourceContains, type RegexResult } from "./regex-result.js";

export interface PageSnapshot {
  readonly url: string;
  readonly status: number;
  readonly redirectLocation: string | null;
  readonly headers: Headers;
  readonly bodyBytes: Uint8Array;
}

export class Page {
  private cachedDocument: HostHtmlDocument | null = null;
  private cachedText: string | null = null;

  constructor(
    private readonly hostApi: HostApi,
    private readonly snapshot: PageSnapshot,
  ) {}

  get url(): string {
    return this.snapshot.url;
  }

  get status(): number {
    return this.snapshot.status;
  }

  get redirectLocation(): string | null {
    return this.snapshot.redirectLocation;
  }

  get headers(): Headers {
    return this.snapshot.headers;
  }

  bodyBytes(): Uint8Array {
    return this.snapshot.bodyBytes;
  }

  body(): string {
    if (this.cachedText === null) {
      this.cachedText = this.hostApi.util.textDecode(this.snapshot.bodyBytes);
    }
    return this.cachedText;
  }

  json<T = unknown>(): T {
    return JSON.parse(this.body()) as T;
  }

  containsHTML(pattern: string | RegExp): boolean {
    return sourceContains(this.body(), pattern);
  }

  regex(pattern: string | RegExp, flags?: string): RegexResult {
    return matchRegex(this.body(), pattern, flags);
  }

  document(): HostHtmlDocument {
    if (this.cachedDocument === null) {
      this.cachedDocument = this.hostApi.html.parse(this.body(), this.snapshot.url);
    }
    return this.cachedDocument;
  }

  find(selector: string): Element[] {
    return this.document()
      .select(selector)
      .map((node) => new Element(node));
  }

  findFirst(selector: string): Element | null {
    const node = this.document().selectFirst(selector);
    return node ? new Element(node) : null;
  }
}
