import type { AccountConfig } from "../account/config.js";
import type { PluginContext } from "../context/context.js";
import type { DownloadLink } from "../types/download-link.js";
import type { FileInfo } from "../types/file-info.js";
import type { FormatInfo } from "../types/format-info.js";
import { compilePattern, matchesAny, type UrlPattern } from "./matching.js";

export type PluginKind = "hoster" | "decrypter";

export interface PluginManifest {
  readonly id: string;
  readonly version: string;
  readonly kind: PluginKind;
  readonly match: readonly UrlPattern[];
}

export interface HosterPluginDefinition {
  readonly id: string;
  readonly version: string;
  readonly match: readonly UrlPattern[];
  readonly account?: AccountConfig;
  checkAvailable?(context: PluginContext): Promise<FileInfo>;
  extract(context: PluginContext): Promise<FormatInfo[]>;
}

export interface DecrypterPluginDefinition {
  readonly id: string;
  readonly version: string;
  readonly match: readonly UrlPattern[];
  decrypt(context: PluginContext): Promise<readonly (string | DownloadLink)[]>;
}

export interface Plugin {
  readonly id: string;
  readonly version: string;
  readonly kind: PluginKind;
  readonly match: readonly UrlPattern[];
  readonly account: AccountConfig | null;
  matches(url: string): boolean;
  checkAvailable?(context: PluginContext): Promise<FileInfo>;
  extract?(context: PluginContext): Promise<FormatInfo[]>;
  decrypt?(context: PluginContext): Promise<readonly DownloadLink[]>;
  manifest(): PluginManifest;
}

function normaliseDecryptResult(
  result: readonly (string | DownloadLink)[],
): DownloadLink[] {
  return result.map((entry) => {
    if (typeof entry === "string") {
      return {
        url: entry,
        filename: null,
        size: null,
        referer: null,
        headers: {},
        properties: {},
      };
    }
    return entry;
  });
}

function validateDefinition(def: { id: string; version: string; match: readonly UrlPattern[] }): void {
  if (!def.id || def.id.trim().length === 0) {
    throw new Error("Plugin definition missing id");
  }
  if (!def.version || def.version.trim().length === 0) {
    throw new Error(`Plugin ${def.id} missing version`);
  }
  if (!Array.isArray(def.match) || def.match.length === 0) {
    throw new Error(`Plugin ${def.id} has no match patterns`);
  }
  for (const pattern of def.match) {
    // Trigger compilation to surface malformed patterns early.
    compilePattern(pattern);
  }
}

export function definePlugin(definition: HosterPluginDefinition): Plugin {
  validateDefinition(definition);
  const base: Plugin = {
    id: definition.id,
    version: definition.version,
    kind: "hoster",
    match: definition.match,
    account: definition.account ?? null,
    matches: (url) => matchesAny(definition.match, url),
    extract: (context) => definition.extract(context),
    manifest: () => ({
      id: definition.id,
      version: definition.version,
      kind: "hoster",
      match: definition.match,
    }),
  };
  if (definition.checkAvailable) {
    base.checkAvailable = (context) => definition.checkAvailable!(context);
  }
  return base;
}

export function defineDecrypter(definition: DecrypterPluginDefinition): Plugin {
  validateDefinition(definition);
  return {
    id: definition.id,
    version: definition.version,
    kind: "decrypter",
    match: definition.match,
    account: null,
    matches: (url) => matchesAny(definition.match, url),
    decrypt: async (context) => normaliseDecryptResult(await definition.decrypt(context)),
    manifest: () => ({
      id: definition.id,
      version: definition.version,
      kind: "decrypter",
      match: definition.match,
    }),
  };
}
