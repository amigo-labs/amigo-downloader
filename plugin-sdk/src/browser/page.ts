import type { HostApi } from "../host/api.js";
import type { HostHtmlDocument } from "../host/types.js";
import { buildFormFromHtml, Form } from "../form/form.js";
import { Element } from "./element.js";
import { Headers } from "./headers.js";
import { matchRegex, sourceContains, type RegexResult } from "./regex-result.js";
import type { Browser } from "./browser.js";

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
    private readonly browser: Browser,
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

  getForms(): Form[] {
    return this.document()
      .select("form")
      .map((node) => buildFormFromHtml(this.hostApi, this.browser, this.snapshot.url, node.html));
  }

  getForm(selector?: string | number): Form | null {
    const forms = this.document().select("form");
    if (selector === undefined) {
      const first = forms[0];
      if (!first) {
        return null;
      }
      return buildFormFromHtml(this.hostApi, this.browser, this.snapshot.url, first.html);
    }
    if (typeof selector === "number") {
      const target = forms[selector];
      if (!target) {
        return null;
      }
      return buildFormFromHtml(this.hostApi, this.browser, this.snapshot.url, target.html);
    }
    const matched = this.document().selectFirst(selector);
    if (!matched) {
      return null;
    }
    return buildFormFromHtml(this.hostApi, this.browser, this.snapshot.url, matched.html);
  }
}
