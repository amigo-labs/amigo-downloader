import { PluginError } from "../errors/plugin-error.js";
import { base64Decode } from "../extraction/encoding.js";
import { getHostApi } from "../host/injection.js";
import { collectTags, parseXmlAttribute, stripXmlNoise } from "./shared.js";

export interface DlcOptions {
  readonly keyExchangeEndpoint: string;
  readonly userAgent?: string;
  readonly signal?: AbortSignal;
}

export interface DlcLink {
  readonly url: string;
  readonly filename: string | null;
  readonly size: number | null;
  readonly password: string | null;
}

export interface DlcContainer {
  readonly packageName: string | null;
  readonly uploadDate: string | null;
  readonly links: readonly DlcLink[];
}

const SERVICE_KEY_LENGTH = 88;

function decodeBase64OrThrow(input: string, reason: string): Uint8Array {
  try {
    return base64Decode(input);
  } catch (cause) {
    throw new PluginError("ContainerDecryptionFailed", { message: reason, cause });
  }
}

function textOf(fragment: string, tagName: string): string | null {
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

function extractKeyFromServiceResponse(body: string): Uint8Array {
  const trimmed = body.trim();
  const match = /<rc>([A-Za-z0-9+/=]+)<\/rc>/i.exec(trimmed);
  const candidate = match?.[1] ?? trimmed;
  return decodeBase64OrThrow(candidate, "DLC service returned invalid key");
}

export async function parse(
  content: string | Uint8Array,
  options: DlcOptions,
): Promise<DlcContainer> {
  const host = getHostApi();
  const rawText =
    typeof content === "string" ? content : host.util.textDecode(content);
  const cleaned = rawText.replace(/\s+/g, "");
  if (cleaned.length <= SERVICE_KEY_LENGTH) {
    throw new PluginError("ContainerDecryptionFailed", {
      message: "DLC payload shorter than service key",
    });
  }
  const payloadBase64 = cleaned.slice(0, cleaned.length - SERVICE_KEY_LENGTH);
  const serviceKey = cleaned.slice(cleaned.length - SERVICE_KEY_LENGTH);

  const response = await host.http({
    method: "POST",
    url: options.keyExchangeEndpoint,
    headers: {
      "Content-Type": "application/x-www-form-urlencoded",
      "User-Agent": options.userAgent ?? "amigo-plugin-sdk",
    },
    body: `jk=${encodeURIComponent(serviceKey)}`,
    followRedirects: true,
    ...(options.signal ? { signal: options.signal } : {}),
  });

  if (response.status < 200 || response.status >= 300) {
    throw new PluginError("ContainerDecryptionFailed", {
      message: `DLC service returned HTTP ${response.status}`,
    });
  }

  const serviceKeyBytes = extractKeyFromServiceResponse(host.util.textDecode(response.body));
  if (serviceKeyBytes.length < 16) {
    throw new PluginError("ContainerDecryptionFailed", {
      message: "DLC service returned key shorter than 16 bytes",
    });
  }
  const aesKey = serviceKeyBytes.slice(0, 16);
  const aesIv = aesKey;

  const payloadBytes = decodeBase64OrThrow(payloadBase64, "DLC payload is not base64");
  let decrypted: Uint8Array;
  try {
    decrypted = host.crypto.aesCbcDecrypt(payloadBytes, aesKey, aesIv);
  } catch (cause) {
    throw new PluginError("ContainerDecryptionFailed", {
      message: "DLC AES decryption failed",
      cause,
    });
  }

  const xml = stripXmlNoise(host.util.textDecode(decodeBase64OrThrow(
    host.util.textDecode(decrypted).trim(),
    "DLC inner payload is not base64",
  )));

  const packageMatch = /<package\b([^>]*)>/.exec(xml);
  const packageAttrs = packageMatch?.[1] ?? "";
  const packageName = parseXmlAttribute(packageAttrs, "name");
  const uploadDate = parseXmlAttribute(packageAttrs, "date");
  const links: DlcLink[] = [];
  for (const tag of collectTags(xml, "file")) {
    const body = (tag[1] ?? "") + ">" + (tag[2] ?? "");
    const url = textOf(body, "url");
    if (!url) {
      continue;
    }
    links.push({
      url,
      filename: textOf(body, "filename"),
      size: parseNumber(textOf(body, "size")),
      password: textOf(body, "password"),
    });
  }

  return { packageName, uploadDate, links };
}
