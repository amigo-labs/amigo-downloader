export interface DownloadLinkInit {
  readonly url: string;
  readonly filename?: string | null;
  readonly size?: number | null;
  readonly referer?: string | null;
  readonly headers?: Readonly<Record<string, string>>;
  readonly properties?: Readonly<Record<string, unknown>>;
}

export interface DownloadLink {
  readonly url: string;
  readonly filename: string | null;
  readonly size: number | null;
  readonly referer: string | null;
  readonly headers: Readonly<Record<string, string>>;
  readonly properties: Readonly<Record<string, unknown>>;
}

export function downloadLink(init: DownloadLinkInit): DownloadLink {
  return {
    url: init.url,
    filename: init.filename ?? null,
    size: init.size ?? null,
    referer: init.referer ?? null,
    headers: init.headers ?? {},
    properties: init.properties ?? {},
  };
}
