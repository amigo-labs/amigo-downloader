import { PluginError } from "../errors/plugin-error.js";
import { getHostApi } from "../host/injection.js";
import {
  CCF_IV,
  CCF_KEY,
  collectTags,
  parseXmlAttribute,
  stripXmlNoise,
} from "./shared.js";

export interface CcfLink {
  readonly url: string;
  readonly filename: string | null;
  readonly size: number | null;
  readonly password: string | null;
}

export interface CcfContainer {
  readonly packageName: string | null;
  readonly links: readonly CcfLink[];
}

function textContent(fragment: string, tagName: string): string | null {
  const pattern = new RegExp(`<${tagName}>([\\s\\S]*?)</${tagName}>`);
  const match = pattern.exec(fragment);
  return match?.[1]?.trim() ?? null;
}

function parseNumber(raw: string | null): number | null {
  if (raw === null) {
    return null;
  }
  const value = Number.parseInt(raw, 10);
  return Number.isNaN(value) ? null : value;
}

export function parse(content: Uint8Array): CcfContainer {
  const host = getHostApi();
  let plain: Uint8Array;
  try {
    plain = host.crypto.aesCbcDecrypt(content, CCF_KEY, CCF_IV);
  } catch (cause) {
    throw new PluginError("ContainerDecryptionFailed", {
      message: "CCF AES decryption failed",
      cause,
    });
  }
  const xml = stripXmlNoise(host.util.textDecode(plain));
  const packageMatch = /<package\b([^>]*)>/.exec(xml);
  const packageName = packageMatch ? parseXmlAttribute(packageMatch[1] ?? "", "name") : null;
  const links: CcfLink[] = [];
  for (const match of collectTags(xml, "link")) {
    const body = (match[1] ?? "") + ">" + (match[2] ?? "");
    const url = textContent(body, "url");
    if (!url) {
      continue;
    }
    links.push({
      url,
      filename: textContent(body, "filename"),
      size: parseNumber(textContent(body, "filesize")),
      password: textContent(body, "password"),
    });
  }
  return { packageName, links };
}
