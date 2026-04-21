import { PluginError } from "../errors/plugin-error.js";
import { getHostApi } from "../host/injection.js";
import type { HostJavascriptEvalOptions } from "../host/types.js";

export interface JavascriptRunOptions extends HostJavascriptEvalOptions {
  readonly skipPermissionCheck?: boolean;
}

function assertPermission(): void {
  if (!getHostApi().permissions.has("javascript_eval")) {
    throw new PluginError("PermissionDenied", {
      message: "javascript_eval permission not granted to this plugin",
    });
  }
}

export async function run<T = unknown>(
  code: string,
  input?: unknown,
  options?: JavascriptRunOptions,
): Promise<T> {
  if (!options?.skipPermissionCheck) {
    assertPermission();
  }
  const host = getHostApi();
  const evalOptions: HostJavascriptEvalOptions = {
    ...(options?.timeoutMilliseconds !== undefined
      ? { timeoutMilliseconds: options.timeoutMilliseconds }
      : {}),
    ...(options?.memoryLimitBytes !== undefined
      ? { memoryLimitBytes: options.memoryLimitBytes }
      : {}),
    ...(input !== undefined ? { input } : {}),
  };
  try {
    return await host.javascript.eval<T>(code, evalOptions);
  } catch (cause) {
    throw new PluginError("EvalError", {
      message: cause instanceof Error ? cause.message : "javascript eval failed",
      cause,
    });
  }
}

const DEAN_EDWARDS_WRAPPER =
  /\beval\(\s*function\s*\(p,\s*a,\s*c,\s*k,\s*e,\s*(d|r)\)\s*\{[\s\S]+?\}\s*\(/;

export async function unpackDeanEdwards(code: string): Promise<string> {
  if (!DEAN_EDWARDS_WRAPPER.test(code)) {
    throw new PluginError("ParseError", {
      message: "input does not look like Dean-Edwards-packed code",
    });
  }
  const patched = code.replace(/^[\s\S]*?\beval\(/, "(");
  return run<string>(`return String((${patched}))`, undefined, {
    timeoutMilliseconds: 5000,
    memoryLimitBytes: 16 * 1024 * 1024,
  });
}

export async function unpackEval(code: string): Promise<string> {
  if (!code.trim().startsWith("eval(")) {
    throw new PluginError("ParseError", {
      message: "input does not start with eval(",
    });
  }
  const trimmed = code.trim();
  const patched = trimmed.replace(/^eval\(/, "String(");
  return run<string>(`return ${patched}`, undefined, {
    timeoutMilliseconds: 5000,
    memoryLimitBytes: 16 * 1024 * 1024,
  });
}
