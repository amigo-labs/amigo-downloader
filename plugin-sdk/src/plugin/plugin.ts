import type { AccountConfig } from "../account/config.js";
import type { PluginContext } from "../context/context.js";
import type { DownloadLink } from "../types/download-link.js";
import type { FileInfo } from "../types/file-info.js";
import type { FormatInfo } from "../types/format-info.js";
import { compilePattern, matchesAny, type UrlPattern } from "./matching.js";

export type PluginKind = "hoster" | "decrypter";

/**
 * Major version of the amigo host API this SDK targets. Mirrors
 * `HOST_API_VERSION` in `crates/plugin-runtime/src/lib.rs`; a plugin built
 * against a major the host does not support is refused at load.
 */
export const HOST_API_VERSION = 1;

/** Capabilities a plugin requests; see `docs/plugin-api.md`. */
export interface PluginPermissions {
  /**
   * Hosts the plugin may reach — exact hosts or one leading wildcard label
   * (`"*.example.com"`). Omit to stay unscoped (any public host).
   */
  readonly domains?: readonly string[];
}

export interface PluginManifest {
  readonly id: string;
  readonly version: string;
  readonly apiVersion: number;
  readonly kind: PluginKind;
  readonly match: readonly UrlPattern[];
  readonly permissions: PluginPermissions;
}

interface CommonDefinition {
  readonly id: string;
  readonly version: string;
  /** Host-API major the plugin is written against. Defaults to {@link HOST_API_VERSION}. */
  readonly apiVersion?: number;
  readonly permissions?: PluginPermissions;
  readonly match: readonly UrlPattern[];
}

export interface HosterPluginDefinition extends CommonDefinition {
  readonly account?: AccountConfig;
  checkAvailable?(context: PluginContext): Promise<FileInfo>;
  extract(context: PluginContext): Promise<FormatInfo[]>;
}

export interface DecrypterPluginDefinition extends CommonDefinition {
  decrypt(context: PluginContext): Promise<readonly (string | DownloadLink)[]>;
}

export interface Plugin {
  readonly id: string;
  readonly version: string;
  readonly apiVersion: number;
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

/** Mirrors `MAX_DOMAIN_ENTRIES` in `crates/plugin-runtime/src/permissions.rs`. */
const MAX_DOMAIN_ENTRIES = 64;

/**
 * Validate one `permissions.domains` entry with the same rules the runtime
 * applies at load (`DomainAllowlist::parse` in
 * `crates/plugin-runtime/src/permissions.rs`), so a plugin the SDK accepts
 * also loads. Returns `null` when valid, else the reason.
 */
export function domainPatternProblem(raw: string): string | null {
  const entry = raw.trim();
  if (entry.length === 0) return "empty";
  if (entry === "*") return 'a bare "*" is not allowed — list the hosts the plugin needs';
  if (entry.includes("://") || entry.includes("/")) return "must be a host, without scheme or path";
  if (entry.includes(":") || entry.includes("@") || entry.includes("[")) {
    return "must be a host name, without port, user info or IP brackets";
  }
  const wildcard = entry.startsWith("*.");
  const rawHost = wildcard ? entry.slice(2) : entry;
  if (rawHost.includes("*")) {
    return '"*" is only allowed as the whole first label ("*.example.com")';
  }
  const host = rawHost.replace(/\.+$/, "").toLowerCase();
  const labels = host.split(".");
  const labelOk = (label: string): boolean =>
    label.length > 0 &&
    label.length <= 63 &&
    !label.startsWith("-") &&
    !label.endsWith("-") &&
    /^[a-z0-9-]+$/.test(label);
  if (host.length > 253 || !labels.every(labelOk)) return "not a valid host name";
  if (wildcard && labels.length < 2) {
    return 'a wildcard must cover at least a registrable domain ("*.example.com")';
  }
  return null;
}

function validateDefinition(def: CommonDefinition): void {
  if (!def.id || def.id.trim().length === 0) {
    throw new Error("Plugin definition missing id");
  }
  if (!def.version || def.version.trim().length === 0) {
    throw new Error(`Plugin ${def.id} missing version`);
  }
  const apiVersion = def.apiVersion ?? HOST_API_VERSION;
  if (!Number.isInteger(apiVersion) || apiVersion < 1 || apiVersion > HOST_API_VERSION) {
    throw new Error(
      `Plugin ${def.id} targets host API ${apiVersion}; this SDK supports up to ${HOST_API_VERSION}`,
    );
  }
  const domains = def.permissions?.domains ?? [];
  if (domains.length > MAX_DOMAIN_ENTRIES) {
    throw new Error(
      `Plugin ${def.id} declares ${domains.length} permissions.domains entries (limit ${MAX_DOMAIN_ENTRIES})`,
    );
  }
  for (const domain of domains) {
    const problem = domainPatternProblem(domain);
    if (problem !== null) {
      throw new Error(`Plugin ${def.id} has invalid permissions.domains entry "${domain}": ${problem}`);
    }
  }
  if (!Array.isArray(def.match) || def.match.length === 0) {
    throw new Error(`Plugin ${def.id} has no match patterns`);
  }
  for (const pattern of def.match) {
    // Trigger compilation to surface malformed patterns early.
    compilePattern(pattern);
  }
}

function manifestOf(definition: CommonDefinition, kind: PluginKind): PluginManifest {
  return {
    id: definition.id,
    version: definition.version,
    apiVersion: definition.apiVersion ?? HOST_API_VERSION,
    kind,
    match: definition.match,
    permissions: definition.permissions ?? {},
  };
}

export function definePlugin(definition: HosterPluginDefinition): Plugin {
  validateDefinition(definition);
  const base: Plugin = {
    id: definition.id,
    version: definition.version,
    apiVersion: definition.apiVersion ?? HOST_API_VERSION,
    kind: "hoster",
    match: definition.match,
    account: definition.account ?? null,
    matches: (url) => matchesAny(definition.match, url),
    extract: (context) => definition.extract(context),
    manifest: () => manifestOf(definition, "hoster"),
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
    apiVersion: definition.apiVersion ?? HOST_API_VERSION,
    kind: "decrypter",
    match: definition.match,
    account: null,
    matches: (url) => matchesAny(definition.match, url),
    decrypt: async (context) => normaliseDecryptResult(await definition.decrypt(context)),
    manifest: () => manifestOf(definition, "decrypter"),
  };
}
