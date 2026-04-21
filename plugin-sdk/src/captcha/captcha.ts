import type { Page } from "../browser/page.js";
import { PluginError } from "../errors/plugin-error.js";
import { getHostApi } from "../host/injection.js";
import type { HostCaptchaApi, HostCaptchaRequest } from "../host/types.js";

export interface CaptchaSolveOptions {
  readonly signal?: AbortSignal;
  readonly invisible?: boolean;
}

export interface CaptchaV2Options extends CaptchaSolveOptions {
  readonly siteKey?: string;
  readonly pageUrl?: string;
}

export interface CaptchaV3Options extends CaptchaSolveOptions {
  readonly action: string;
  readonly siteKey?: string;
  readonly pageUrl?: string;
}

export interface CaptchaImageOptions extends CaptchaSolveOptions {
  readonly mode?: "text" | "math";
}

export interface CaptchaResult {
  readonly token: string;
  readonly jobId: string | null;
  reportFailed(): Promise<void>;
}

function hostCaptcha(): HostCaptchaApi {
  return getHostApi().captcha;
}

async function solve(request: HostCaptchaRequest): Promise<CaptchaResult> {
  const captcha = hostCaptcha();
  const result = await captcha.solve(request);
  return {
    token: result.token,
    jobId: result.jobId ?? null,
    async reportFailed() {
      if (!captcha.reportFailed || !result.jobId) {
        return;
      }
      await captcha.reportFailed(result.jobId);
    },
  };
}

function buildRequest(
  base: HostCaptchaRequest,
  options: CaptchaSolveOptions | undefined,
): HostCaptchaRequest {
  return {
    ...base,
    ...(options?.signal ? { signal: options.signal } : {}),
    ...(options && "invisible" in options && options.invisible !== undefined
      ? { invisible: options.invisible }
      : {}),
  };
}

function extractSiteKey(page: Page, selectors: readonly string[]): string | null {
  for (const selector of selectors) {
    const element = page.findFirst(selector);
    if (!element) {
      continue;
    }
    const key = element.attr("data-sitekey");
    if (key) {
      return key;
    }
  }
  return null;
}

export async function recaptchaV2(page: Page, options?: CaptchaV2Options): Promise<CaptchaResult> {
  const siteKey =
    options?.siteKey ??
    extractSiteKey(page, [
      ".g-recaptcha",
      "[data-sitekey].g-recaptcha",
      "iframe[src*=\"recaptcha\"]",
    ]);
  if (!siteKey) {
    throw new PluginError("CaptchaFailed", { message: "reCaptcha v2 site key not found" });
  }
  return solve(
    buildRequest(
      { kind: "recaptcha_v2", siteKey, pageUrl: options?.pageUrl ?? page.url },
      options,
    ),
  );
}

export async function recaptchaV3(page: Page, options: CaptchaV3Options): Promise<CaptchaResult> {
  const siteKey =
    options.siteKey ?? extractSiteKey(page, [".g-recaptcha", "[data-sitekey]"]);
  if (!siteKey) {
    throw new PluginError("CaptchaFailed", { message: "reCaptcha v3 site key not found" });
  }
  return solve(
    buildRequest(
      {
        kind: "recaptcha_v3",
        siteKey,
        pageUrl: options.pageUrl ?? page.url,
        action: options.action,
      },
      options,
    ),
  );
}

export async function hcaptcha(page: Page, options?: CaptchaV2Options): Promise<CaptchaResult> {
  const siteKey =
    options?.siteKey ?? extractSiteKey(page, [".h-captcha", "[data-sitekey].h-captcha"]);
  if (!siteKey) {
    throw new PluginError("CaptchaFailed", { message: "hCaptcha site key not found" });
  }
  return solve(
    buildRequest(
      { kind: "hcaptcha", siteKey, pageUrl: options?.pageUrl ?? page.url },
      options,
    ),
  );
}

export async function turnstile(page: Page, options?: CaptchaV2Options): Promise<CaptchaResult> {
  const siteKey =
    options?.siteKey ?? extractSiteKey(page, [".cf-turnstile", "[data-sitekey].cf-turnstile"]);
  if (!siteKey) {
    throw new PluginError("CaptchaFailed", { message: "Turnstile site key not found" });
  }
  return solve(
    buildRequest(
      { kind: "turnstile", siteKey, pageUrl: options?.pageUrl ?? page.url },
      options,
    ),
  );
}

export async function image(imageUrl: string, options?: CaptchaImageOptions): Promise<CaptchaResult> {
  return solve(
    buildRequest(
      { kind: "image", imageUrl, ...(options?.mode ? { mode: options.mode } : {}) },
      options,
    ),
  );
}

export async function interactive(
  prompt: string,
  imageUrl?: string,
  options?: CaptchaSolveOptions,
): Promise<CaptchaResult> {
  return solve(
    buildRequest(
      { kind: "interactive", prompt, ...(imageUrl ? { imageUrl } : {}) },
      options,
    ),
  );
}

export interface AutoDetection {
  readonly kind: HostCaptchaRequest["kind"];
  readonly siteKey: string;
}

export function detect(page: Page): AutoDetection | null {
  const cases: Array<{ kind: HostCaptchaRequest["kind"]; selectors: string[] }> = [
    { kind: "turnstile", selectors: [".cf-turnstile"] },
    { kind: "hcaptcha", selectors: [".h-captcha"] },
    { kind: "recaptcha_v2", selectors: [".g-recaptcha"] },
  ];
  for (const entry of cases) {
    const key = extractSiteKey(page, entry.selectors);
    if (key) {
      return { kind: entry.kind, siteKey: key };
    }
  }
  return null;
}

export async function auto(page: Page, options?: CaptchaSolveOptions): Promise<CaptchaResult> {
  const detection = detect(page);
  if (!detection) {
    throw new PluginError("CaptchaFailed", {
      message: "No captcha widget detected on page",
    });
  }
  return solve(
    buildRequest(
      { kind: detection.kind, siteKey: detection.siteKey, pageUrl: page.url },
      options,
    ),
  );
}
