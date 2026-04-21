import { hexDecode } from "../extraction/encoding.js";

// Well-known RSDF constants. Historically published in JDownloader and pyLoad.
// The "key" originally shipped as 24 hex bytes but the first 16 bytes are the
// effective AES-128 key; the trailing zeros are padding.
export const RSDF_KEY = hexDecode("8C35192D964DC3182C6F84F3252239EB");
export const RSDF_IV = hexDecode("A3D5A33CB95AC1F5CBDB1AD25CB0A7AA");

// CCF constants as published in public implementations (pyLoad/JDownloader).
export const CCF_KEY = hexDecode("98456F2EA6360FBC8E2EAD2D9A1F4A5C");
export const CCF_IV = hexDecode("00000000000000000000000000000000");

export function stripXmlNoise(xml: string): string {
  return xml.replace(/<!--[\s\S]*?-->/g, "").replace(/﻿/g, "").trim();
}

export function parseXmlAttribute(tag: string, attribute: string): string | null {
  const pattern = new RegExp(`\\b${attribute}="([^"]*)"`);
  const match = pattern.exec(tag);
  return match?.[1] ?? null;
}

export function collectTags(source: string, tagName: string): RegExpMatchArray[] {
  const pattern = new RegExp(
    `<${tagName}\\b([^>]*)(?:/>|>([\\s\\S]*?)</${tagName}>)`,
    "g",
  );
  return Array.from(source.matchAll(pattern));
}
