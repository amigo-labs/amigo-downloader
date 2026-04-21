import { getHostApi } from "../host/injection.js";

export type ContainerKind = "dlc" | "ccf" | "rsdf";

export function detect(content: Uint8Array): ContainerKind | null {
  if (content.length === 0) {
    return null;
  }
  const sampleLength = Math.min(content.length, 1024);
  const sample = content.slice(0, sampleLength);
  const text = getHostApi().util.textDecode(sample).trim();

  if (/^[A-Za-z0-9+/=\s]+$/.test(text) && text.length >= 100) {
    // DLC payloads are large base64 blobs followed by a 88-char service key.
    if (text.replace(/\s+/g, "").length >= 100) {
      return "dlc";
    }
  }
  if (/^[0-9a-fA-F\s]+$/.test(text) && text.replace(/\s+/g, "").length % 2 === 0) {
    return "rsdf";
  }
  const magic = new Uint8Array(sample.slice(0, 2));
  if (magic.length >= 2 && magic[0] === 0xcc && magic[1] === 0xff) {
    return "ccf";
  }
  return null;
}
