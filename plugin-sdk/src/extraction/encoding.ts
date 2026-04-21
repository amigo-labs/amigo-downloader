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

export function base64Encode(input: string | Uint8Array): string {
  const bytes = typeof input === "string" ? new TextEncoder().encode(input) : input;
  let binary = "";
  for (let index = 0; index < bytes.length; index += 1) {
    binary += String.fromCharCode(bytes[index]!);
  }
  return btoa(binary);
}

export function base64Decode(input: string): Uint8Array {
  const binary = atob(input);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
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
