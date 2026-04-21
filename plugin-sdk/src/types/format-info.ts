export type MediaType = "direct" | "hls" | "dash";

export interface FormatInfoInit {
  readonly url: string;
  readonly filename?: string | null;
  readonly size?: number | null;
  readonly headers?: Readonly<Record<string, string>>;
  readonly quality?: string | null;
  readonly bandwidth?: number | null;
  readonly codec?: string | null;
  readonly width?: number | null;
  readonly height?: number | null;
  readonly mediaType?: MediaType;
  readonly manifestUrl?: string | null;
  readonly mimeType?: string | null;
  readonly properties?: Readonly<Record<string, unknown>>;
}

export interface FormatInfo {
  readonly url: string;
  readonly filename: string | null;
  readonly size: number | null;
  readonly headers: Readonly<Record<string, string>>;
  readonly quality: string | null;
  readonly bandwidth: number | null;
  readonly codec: string | null;
  readonly width: number | null;
  readonly height: number | null;
  readonly mediaType: MediaType;
  readonly manifestUrl: string | null;
  readonly mimeType: string | null;
  readonly properties: Readonly<Record<string, unknown>>;
}

export function formatInfo(init: FormatInfoInit): FormatInfo {
  return {
    url: init.url,
    filename: init.filename ?? null,
    size: init.size ?? null,
    headers: init.headers ?? {},
    quality: init.quality ?? null,
    bandwidth: init.bandwidth ?? null,
    codec: init.codec ?? null,
    width: init.width ?? null,
    height: init.height ?? null,
    mediaType: init.mediaType ?? "direct",
    manifestUrl: init.manifestUrl ?? null,
    mimeType: init.mimeType ?? null,
    properties: init.properties ?? {},
  };
}
