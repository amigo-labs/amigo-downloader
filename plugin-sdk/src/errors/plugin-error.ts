import type { ErrorCode } from "./codes.js";

export interface PluginErrorOptions {
  readonly message?: string;
  readonly retryAfterMilliseconds?: number;
  readonly cause?: unknown;
  readonly details?: Readonly<Record<string, unknown>>;
}

export interface SerializedPluginError {
  readonly code: ErrorCode;
  readonly message: string;
  readonly retryAfterMilliseconds: number | null;
  readonly details: Readonly<Record<string, unknown>> | null;
  readonly stack: string | null;
}

export class PluginError extends Error {
  readonly code: ErrorCode;
  readonly retryAfterMilliseconds: number | null;
  readonly details: Readonly<Record<string, unknown>> | null;

  constructor(code: ErrorCode, options: PluginErrorOptions = {}) {
    super(options.message ?? code, options.cause !== undefined ? { cause: options.cause } : undefined);
    this.name = "PluginError";
    this.code = code;
    this.retryAfterMilliseconds = options.retryAfterMilliseconds ?? null;
    this.details = options.details ?? null;
  }

  serialize(): SerializedPluginError {
    return {
      code: this.code,
      message: this.message,
      retryAfterMilliseconds: this.retryAfterMilliseconds,
      details: this.details,
      stack: this.stack ?? null,
    };
  }
}

export function isPluginError(value: unknown): value is PluginError {
  return value instanceof PluginError;
}

export function toPluginError(value: unknown): PluginError {
  if (isPluginError(value)) {
    return value;
  }
  if (value instanceof Error) {
    const error = new PluginError("PluginDefect", {
      message: value.message,
      cause: value,
    });
    if (value.stack !== undefined) {
      error.stack = value.stack;
    }
    return error;
  }
  return new PluginError("PluginDefect", {
    message: typeof value === "string" ? value : "Unknown plugin error",
    cause: value,
  });
}
