import type { Browser, BrowserRequestOptions } from "../browser/browser.js";
import type { Page } from "../browser/page.js";
import type { HostApi } from "../host/api.js";
import type { HttpMethod } from "../host/types.js";

export type FormSubmitOptions = Omit<BrowserRequestOptions, "url" | "method" | "body">;

export class Form {
  readonly action: string;
  readonly method: HttpMethod;
  private readonly fields = new Map<string, string>();

  constructor(
    private readonly browser: Browser,
    options: { action: string; method: HttpMethod; inputs: Iterable<readonly [string, string]> },
  ) {
    this.action = options.action;
    this.method = options.method;
    for (const [name, value] of options.inputs) {
      this.fields.set(name, value);
    }
  }

  put(name: string, value: string): void {
    this.fields.set(name, value);
  }

  get(name: string): string | null {
    return this.fields.get(name) ?? null;
  }

  has(name: string): boolean {
    return this.fields.has(name);
  }

  remove(name: string): void {
    this.fields.delete(name);
  }

  names(): string[] {
    return Array.from(this.fields.keys());
  }

  toRecord(): Record<string, string> {
    return Object.fromEntries(this.fields);
  }

  submit(
    overrides?: Readonly<Record<string, string>>,
    options?: FormSubmitOptions,
  ): Promise<Page> {
    const data = { ...this.toRecord(), ...(overrides ?? {}) };
    if (this.method === "GET" || this.method === "HEAD") {
      const url = new URL(this.action);
      for (const [name, value] of Object.entries(data)) {
        url.searchParams.set(name, value);
      }
      const requestOptions = options ?? {};
      return this.method === "HEAD"
        ? this.browser.headPage(url.toString(), requestOptions)
        : this.browser.getPage(url.toString(), requestOptions);
    }
    return this.browser.postPage(this.action, data, options);
  }
}

function parseMethod(raw: string | null): HttpMethod {
  if (raw === null) {
    return "GET";
  }
  const upper = raw.toUpperCase();
  switch (upper) {
    case "GET":
    case "POST":
    case "PUT":
    case "PATCH":
    case "DELETE":
    case "HEAD":
    case "OPTIONS":
      return upper;
    default:
      return "GET";
  }
}

const INPUT_TAGS = new Set(["input", "select", "textarea"]);

function attributeValue(
  html: string,
  tag: string,
  start: number,
  end: number,
  attribute: string,
): string | null {
  const segment = html.slice(start, end);
  const pattern = new RegExp(`\\b${attribute}\\s*=\\s*(?:"([^"]*)"|'([^']*)'|([^\\s>]+))`, "i");
  const match = pattern.exec(segment);
  if (!match) {
    return null;
  }
  return match[1] ?? match[2] ?? match[3] ?? null;
}

interface RawInput {
  readonly tag: string;
  readonly attrs: Record<string, string | null>;
}

function parseInputs(html: string): RawInput[] {
  const results: RawInput[] = [];
  const tagPattern = /<(input|select|textarea)\b([^>]*)>/gi;
  for (const match of html.matchAll(tagPattern)) {
    const tag = match[1]!.toLowerCase();
    if (!INPUT_TAGS.has(tag)) {
      continue;
    }
    const attrString = match[2] ?? "";
    const attrs: Record<string, string | null> = {};
    const attrPattern = /([a-zA-Z_:][-a-zA-Z0-9_:.]*)\s*(?:=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+)))?/g;
    for (const attr of attrString.matchAll(attrPattern)) {
      const name = attr[1]!.toLowerCase();
      const value = attr[2] ?? attr[3] ?? attr[4] ?? null;
      attrs[name] = value;
    }
    results.push({ tag, attrs });
  }
  return results;
}

function resolveAction(baseUrl: string, action: string | null): string {
  if (!action || action.length === 0) {
    return baseUrl;
  }
  return new URL(action, baseUrl).toString();
}

export function buildFormFromHtml(
  _hostApi: HostApi,
  browser: Browser,
  baseUrl: string,
  formHtml: string,
): Form {
  const action = attributeValue(formHtml, "form", 0, formHtml.length, "action");
  const method = attributeValue(formHtml, "form", 0, formHtml.length, "method");
  const inputs: Array<readonly [string, string]> = [];
  for (const input of parseInputs(formHtml)) {
    const name = input.attrs["name"];
    if (typeof name !== "string" || name.length === 0) {
      continue;
    }
    const type = input.attrs["type"]?.toLowerCase() ?? "text";
    if (input.tag === "input" && (type === "checkbox" || type === "radio")) {
      if (input.attrs["checked"] === undefined) {
        continue;
      }
    }
    if (input.tag === "input" && (type === "submit" || type === "button" || type === "image")) {
      continue;
    }
    const value = input.attrs["value"] ?? "";
    inputs.push([name, value]);
  }
  return new Form(browser, {
    action: resolveAction(baseUrl, action),
    method: parseMethod(method),
    inputs,
  });
}
