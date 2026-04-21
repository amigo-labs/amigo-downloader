import { getHostApi } from "../host/injection.js";
import type { HostHtmlDocument } from "../host/types.js";

export function parse(source: string, baseUrl?: string): HostHtmlDocument {
  return getHostApi().html.parse(source, baseUrl);
}

export function stripTags(source: string): string {
  return source.replace(/<script[\s\S]*?<\/script>/gi, "")
    .replace(/<style[\s\S]*?<\/style>/gi, "")
    .replace(/<[^>]+>/g, "")
    .replace(/\s+/g, " ")
    .trim();
}
