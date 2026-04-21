import type { ErrorCode } from "./codes.js";
import { PluginError, type PluginErrorOptions } from "./plugin-error.js";

function raise(code: ErrorCode, options?: PluginErrorOptions): never {
  throw new PluginError(code, options);
}

export function fileNotFound(options?: PluginErrorOptions): never {
  raise("FileNotFound", options);
}

export function pluginDefect(options?: PluginErrorOptions): never {
  raise("PluginDefect", options);
}

export function premiumOnly(options?: PluginErrorOptions): never {
  raise("PremiumOnly", options);
}

export function temporarilyUnavailable(options?: PluginErrorOptions): never {
  raise("TemporarilyUnavailable", options);
}

export function ipBlocked(options?: PluginErrorOptions): never {
  raise("IpBlocked", options);
}

export function captchaFailed(options?: PluginErrorOptions): never {
  raise("CaptchaFailed", options);
}

export function captchaUnsolvable(options?: PluginErrorOptions): never {
  raise("CaptchaUnsolvable", options);
}

export function hosterUnavailable(options?: PluginErrorOptions): never {
  raise("HosterUnavailable", options);
}

export function downloadLimitReached(options?: PluginErrorOptions): never {
  raise("DownloadLimitReached", options);
}

export function authFailed(options?: PluginErrorOptions): never {
  raise("AuthFailed", options);
}

export function authRequired(options?: PluginErrorOptions): never {
  raise("AuthRequired", options);
}

export function fatal(options?: PluginErrorOptions): never {
  raise("Fatal", options);
}

export function retry(options?: PluginErrorOptions): never {
  raise("Retry", options);
}

export function httpError(status: number, options?: PluginErrorOptions): never {
  raise("HttpError", {
    ...options,
    details: { ...(options?.details ?? {}), status },
  });
}

export function timeoutError(options?: PluginErrorOptions): never {
  raise("TimeoutError", options);
}

export function abortError(options?: PluginErrorOptions): never {
  raise("AbortError", options);
}

export function parseError(options?: PluginErrorOptions): never {
  raise("ParseError", options);
}

export function budgetExceeded(options?: PluginErrorOptions): never {
  raise("BudgetExceeded", options);
}

export function permissionDenied(options?: PluginErrorOptions): never {
  raise("PermissionDenied", options);
}

export function bodyTooLarge(options?: PluginErrorOptions): never {
  raise("BodyTooLarge", options);
}

export function evalError(options?: PluginErrorOptions): never {
  raise("EvalError", options);
}

export function containerDecryptionFailed(options?: PluginErrorOptions): never {
  raise("ContainerDecryptionFailed", options);
}

export function manifestParseError(options?: PluginErrorOptions): never {
  raise("ManifestParseError", options);
}
