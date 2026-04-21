export const ERROR_CODES = [
  "FileNotFound",
  "PluginDefect",
  "PremiumOnly",
  "TemporarilyUnavailable",
  "IpBlocked",
  "CaptchaFailed",
  "CaptchaUnsolvable",
  "HosterUnavailable",
  "DownloadLimitReached",
  "AuthFailed",
  "AuthRequired",
  "Fatal",
  "Retry",
  "HttpError",
  "TimeoutError",
  "AbortError",
  "ParseError",
  "BudgetExceeded",
  "PermissionDenied",
  "BodyTooLarge",
  "EvalError",
  "ContainerDecryptionFailed",
  "ManifestParseError",
] as const;

export type ErrorCode = (typeof ERROR_CODES)[number];

export function isErrorCode(value: unknown): value is ErrorCode {
  return typeof value === "string" && (ERROR_CODES as readonly string[]).includes(value);
}
