const namedEntities: Record<string, string> = {
  amp: "&",
  lt: "<",
  gt: ">",
  quot: '"',
  apos: "'",
  nbsp: " ",
  copy: "©",
  reg: "®",
  trade: "™",
  hellip: "…",
  mdash: "—",
  ndash: "–",
  laquo: "«",
  raquo: "»",
  lsquo: "‘",
  rsquo: "’",
  ldquo: "“",
  rdquo: "”",
  Auml: "Ä",
  Ouml: "Ö",
  Uuml: "Ü",
  auml: "ä",
  ouml: "ö",
  uuml: "ü",
  szlig: "ß",
};

export function htmlDecode(input: string): string {
  return input.replace(/&(#x[0-9a-fA-F]+|#[0-9]+|[a-zA-Z][a-zA-Z0-9]*);/g, (match, entity: string) => {
    if (entity.startsWith("#x") || entity.startsWith("#X")) {
      const codePoint = Number.parseInt(entity.slice(2), 16);
      return Number.isFinite(codePoint) ? String.fromCodePoint(codePoint) : match;
    }
    if (entity.startsWith("#")) {
      const codePoint = Number.parseInt(entity.slice(1), 10);
      return Number.isFinite(codePoint) ? String.fromCodePoint(codePoint) : match;
    }
    const replacement = namedEntities[entity];
    return replacement ?? match;
  });
}

export function htmlEncode(input: string): string {
  return input
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

export function unicodeDecode(input: string): string {
  return input
    .replace(/\\u\{([0-9a-fA-F]+)\}/g, (_, hex: string) =>
      String.fromCodePoint(Number.parseInt(hex, 16)),
    )
    .replace(/\\u([0-9a-fA-F]{4})/g, (_, hex: string) =>
      String.fromCharCode(Number.parseInt(hex, 16)),
    )
    .replace(/\\x([0-9a-fA-F]{2})/g, (_, hex: string) =>
      String.fromCharCode(Number.parseInt(hex, 16)),
    );
}

export function urlEncode(input: string): string {
  return encodeURIComponent(input);
}

export function urlDecode(input: string): string {
  return decodeURIComponent(input);
}

const BASE64_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const BASE64_REVERSE: Record<string, number> = (() => {
  const table: Record<string, number> = {};
  for (let index = 0; index < BASE64_ALPHABET.length; index += 1) {
    table[BASE64_ALPHABET[index]!] = index;
  }
  return table;
})();

export function base64Encode(input: string | Uint8Array): string {
  const bytes = typeof input === "string" ? new TextEncoder().encode(input) : input;
  let output = "";
  for (let index = 0; index < bytes.length; index += 3) {
    const b0 = bytes[index]!;
    const b1 = index + 1 < bytes.length ? bytes[index + 1]! : -1;
    const b2 = index + 2 < bytes.length ? bytes[index + 2]! : -1;
    const triplet =
      (b0 << 16) | ((b1 >= 0 ? b1 : 0) << 8) | (b2 >= 0 ? b2 : 0);
    output += BASE64_ALPHABET[(triplet >> 18) & 0x3f]!;
    output += BASE64_ALPHABET[(triplet >> 12) & 0x3f]!;
    output += b1 >= 0 ? BASE64_ALPHABET[(triplet >> 6) & 0x3f]! : "=";
    output += b2 >= 0 ? BASE64_ALPHABET[triplet & 0x3f]! : "=";
  }
  return output;
}

export function base64Decode(input: string): Uint8Array {
  const cleaned = input.replace(/\s+/g, "");
  const unpadded = cleaned.replace(/=+$/, "");
  const bytes = new Uint8Array(Math.floor((unpadded.length * 3) / 4));
  let writeIndex = 0;
  for (let index = 0; index < unpadded.length; index += 4) {
    const a = BASE64_REVERSE[unpadded[index]!];
    const b = BASE64_REVERSE[unpadded[index + 1]!];
    const c = unpadded[index + 2] !== undefined ? BASE64_REVERSE[unpadded[index + 2]!] : 0;
    const d = unpadded[index + 3] !== undefined ? BASE64_REVERSE[unpadded[index + 3]!] : 0;
    if (a === undefined || b === undefined) {
      throw new Error("base64Decode: invalid character");
    }
    const chunk = (a << 18) | (b << 12) | ((c ?? 0) << 6) | (d ?? 0);
    bytes[writeIndex++] = (chunk >> 16) & 0xff;
    if (unpadded[index + 2] !== undefined) {
      bytes[writeIndex++] = (chunk >> 8) & 0xff;
    }
    if (unpadded[index + 3] !== undefined) {
      bytes[writeIndex++] = chunk & 0xff;
    }
  }
  return bytes.slice(0, writeIndex);
}

export function base64DecodeToString(input: string): string {
  return new TextDecoder("utf-8").decode(base64Decode(input));
}

export function hexEncode(input: string | Uint8Array): string {
  const bytes = typeof input === "string" ? new TextEncoder().encode(input) : input;
  let hex = "";
  for (let index = 0; index < bytes.length; index += 1) {
    hex += bytes[index]!.toString(16).padStart(2, "0");
  }
  return hex;
}

export function hexDecode(input: string): Uint8Array {
  if (input.length % 2 !== 0) {
    throw new Error("hexDecode: odd-length input");
  }
  const bytes = new Uint8Array(input.length / 2);
  for (let index = 0; index < bytes.length; index += 1) {
    const byte = Number.parseInt(input.slice(index * 2, index * 2 + 2), 16);
    if (Number.isNaN(byte)) {
      throw new Error(`hexDecode: invalid byte at index ${index}`);
    }
    bytes[index] = byte;
  }
  return bytes;
}
