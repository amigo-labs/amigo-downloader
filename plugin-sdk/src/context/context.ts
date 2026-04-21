import { Browser } from "../browser/browser.js";
import { CookieJar } from "../browser/cookies.js";
import { getHostApi } from "../host/injection.js";
import type { HostApi } from "../host/api.js";
import type { DownloadLink, DownloadLinkInit } from "../types/download-link.js";
import { downloadLink } from "../types/download-link.js";
import type { FormatInfo, FormatInfoInit } from "../types/format-info.js";
import { formatInfo } from "../types/format-info.js";
import type { PluginConfig } from "../types/plugin-config.js";
import { pluginConfig } from "../types/plugin-config.js";

export type LogLevel = "trace" | "debug" | "info" | "warn" | "error";

export interface LogRecord {
  readonly level: LogLevel;
  readonly message: string;
  readonly metadata: Readonly<Record<string, unknown>> | null;
  readonly timestamp: number;
}

export interface ProgressRecord {
  readonly current: number;
  readonly total: number | null;
  readonly message: string | null;
  readonly timestamp: number;
}

export interface AccountContext {
  readonly id: string;
  readonly premium: boolean;
  readonly session: Readonly<Record<string, unknown>>;
}

export interface PluginContextInit {
  readonly url: string;
  readonly browser?: Browser;
  readonly account?: AccountContext;
  readonly config?: PluginConfig;
  readonly signal?: AbortSignal;
  readonly hostApi?: HostApi;
  readonly onLog?: (record: LogRecord) => void;
  readonly onProgress?: (record: ProgressRecord) => void;
}

export interface PluginContext {
  readonly url: string;
  readonly browser: Browser;
  readonly account: AccountContext | null;
  readonly config: PluginConfig;
  readonly abortSignal: AbortSignal | null;
  log(level: LogLevel, message: string, metadata?: Readonly<Record<string, unknown>>): void;
  wait(milliseconds: number): Promise<void>;
  progress(current: number, total?: number, message?: string): void;
  link(init: DownloadLinkInit | string): DownloadLink;
  format(init: FormatInfoInit): FormatInfo;
  formats(inits: readonly FormatInfoInit[]): FormatInfo[];
}

export function createPluginContext(init: PluginContextInit): PluginContext {
  const hostApi = init.hostApi ?? getHostApi();
  const browser =
    init.browser ??
    new Browser({
      hostApi,
      cookies: new CookieJar(),
    });
  const config = init.config ?? pluginConfig({});
  const signal = init.signal ?? null;

  return {
    url: init.url,
    browser,
    account: init.account ?? null,
    config,
    abortSignal: signal,
    log(level, message, metadata) {
      init.onLog?.({
        level,
        message,
        metadata: metadata ?? null,
        timestamp: hostApi.util.now(),
      });
    },
    wait(milliseconds) {
      return hostApi.util.sleep(milliseconds, signal ?? undefined);
    },
    progress(current, total, message) {
      init.onProgress?.({
        current,
        total: total ?? null,
        message: message ?? null,
        timestamp: hostApi.util.now(),
      });
    },
    link(input) {
      if (typeof input === "string") {
        return downloadLink({ url: input });
      }
      return downloadLink(input);
    },
    format(formatInit) {
      return formatInfo(formatInit);
    },
    formats(inits) {
      return inits.map((entry) => formatInfo(entry));
    },
  };
}
