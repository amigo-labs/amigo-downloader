import { PluginError } from "../errors/plugin-error.js";
import { base64Decode, hexDecode } from "../extraction/encoding.js";
import { getHostApi } from "../host/injection.js";
import { RSDF_IV, RSDF_KEY } from "./shared.js";

function asBytes(content: string | Uint8Array): Uint8Array {
  if (typeof content === "string") {
    const cleaned = content.replace(/\s+/g, "");
    try {
      return hexDecode(cleaned);
    } catch (cause) {
      throw new PluginError("ContainerDecryptionFailed", {
        message: "RSDF content is not valid hex",
        cause,
      });
    }
  }
  return content;
}

export function parse(content: string | Uint8Array): string[] {
  const host = getHostApi();
  const bytes = asBytes(content);
  let plain: Uint8Array;
  try {
    plain = host.crypto.aesCbcDecrypt(bytes, RSDF_KEY, RSDF_IV);
  } catch (cause) {
    throw new PluginError("ContainerDecryptionFailed", {
      message: "RSDF AES decryption failed",
      cause,
    });
  }
  const text = host.util.textDecode(plain);
  const lines = text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
  const urls: string[] = [];
  for (const line of lines) {
    try {
      const decoded = host.util.textDecode(base64Decode(line));
      if (decoded.length > 0) {
        urls.push(decoded);
      }
    } catch (cause) {
      throw new PluginError("ContainerDecryptionFailed", {
        message: "RSDF line is not valid base64",
        cause,
      });
    }
  }
  return urls;
}
