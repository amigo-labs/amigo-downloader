import { getHostApi } from "../host/injection.js";

export type ContainerKind = "dlc" | "ccf" | "rsdf";

export function detect(content: Uint8Array): ContainerKind | null {
  if (content.length === 0) {
    return null;
  }
  const sampleLength = Math.min(content.length, 1024);
  const sample = content.slice(0, sampleLength);

  // CCF: binary magic bytes 0xCC 0xFF — the most specific signal, check first.
  if (sample.length >= 2 && sample[0] === 0xcc && sample[1] === 0xff) {
    return "ccf";
  }

  const text = getHostApi().util.textDecode(sample).trim();
  const compact = text.replace(/\s+/g, "");

  // RSDF is hex-encoded. Hex is a strict subset of the base64 charset, so it
  // MUST be tested before the DLC (base64) branch — otherwise every RSDF file
  // matches the base64 pattern first and is misclassified as DLC. A real DLC
  // base64 blob contains non-hex characters (uppercase G-Z, +, /, =) and so
  // still falls through to the DLC branch below.
  if (compact.length % 2 === 0 && /^[0-9a-fA-F\s]+$/.test(text)) {
    return "rsdf";
  }

  // DLC: a large base64 blob.
  if (compact.length >= 100 && /^[A-Za-z0-9+/=\s]+$/.test(text)) {
    return "dlc";
  }

  return null;
}
