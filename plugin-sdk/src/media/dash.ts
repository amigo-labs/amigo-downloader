export interface DashRepresentation {
  readonly id: string;
  readonly bandwidth: number;
  readonly width: number | null;
  readonly height: number | null;
  readonly codecs: string | null;
  readonly mimeType: string | null;
  readonly baseUrl: string | null;
  readonly frameRate: string | null;
}

export type DashContentType = "video" | "audio" | "text" | "image" | "unknown";

export interface DashAdaptationSet {
  readonly id: string | null;
  readonly mimeType: string | null;
  readonly contentType: DashContentType;
  readonly language: string | null;
  readonly representations: readonly DashRepresentation[];
}

export interface DashPeriod {
  readonly id: string | null;
  readonly start: string | null;
  readonly duration: string | null;
  readonly adaptationSets: readonly DashAdaptationSet[];
}

export interface DashManifest {
  readonly type: "static" | "dynamic";
  readonly duration: string | null;
  readonly minBufferTime: string | null;
  readonly periods: readonly DashPeriod[];
}

function collect(source: string, pattern: RegExp): RegExpMatchArray[] {
  return Array.from(source.matchAll(pattern));
}

function attribute(tag: string, name: string): string | null {
  const pattern = new RegExp(`\\b${name}="([^"]*)"`);
  const match = pattern.exec(tag);
  return match?.[1] ?? null;
}

function parseRepresentation(fragment: string): DashRepresentation {
  const id = attribute(fragment, "id") ?? "";
  const bandwidth = Number.parseInt(attribute(fragment, "bandwidth") ?? "0", 10) || 0;
  const width = attribute(fragment, "width");
  const height = attribute(fragment, "height");
  const frameRate = attribute(fragment, "frameRate");
  const codecs = attribute(fragment, "codecs");
  const mimeType = attribute(fragment, "mimeType");
  const baseUrlMatch = /<BaseURL>([^<]+)<\/BaseURL>/.exec(fragment);
  const baseUrl = baseUrlMatch?.[1] ?? null;
  return {
    id,
    bandwidth,
    width: width ? Number.parseInt(width, 10) : null,
    height: height ? Number.parseInt(height, 10) : null,
    codecs,
    mimeType,
    baseUrl,
    frameRate,
  };
}

function parseAdaptationSet(fragment: string): DashAdaptationSet {
  const id = attribute(fragment, "id");
  const mimeType = attribute(fragment, "mimeType");
  const language = attribute(fragment, "lang");
  const contentTypeRaw = (
    attribute(fragment, "contentType") ?? mimeType ?? ""
  ).toLowerCase();
  let contentType: DashContentType = "unknown";
  if (contentTypeRaw.includes("video")) {
    contentType = "video";
  } else if (contentTypeRaw.includes("audio")) {
    contentType = "audio";
  } else if (contentTypeRaw.includes("text")) {
    contentType = "text";
  } else if (contentTypeRaw.includes("image")) {
    contentType = "image";
  }

  const reprPattern = /<Representation\b([^>]*)(?:\/>|>([\s\S]*?)<\/Representation>)/g;
  const representations: DashRepresentation[] = [];
  for (const match of collect(fragment, reprPattern)) {
    const opening = match[1] ?? "";
    const body = match[2] ?? "";
    representations.push(parseRepresentation(opening + body));
  }
  return { id, mimeType, contentType, language, representations };
}

function parsePeriod(fragment: string): DashPeriod {
  const id = attribute(fragment, "id");
  const start = attribute(fragment, "start");
  const duration = attribute(fragment, "duration");
  const adaptationPattern =
    /<AdaptationSet\b([^>]*)>([\s\S]*?)<\/AdaptationSet>/g;
  const adaptationSets: DashAdaptationSet[] = [];
  for (const match of collect(fragment, adaptationPattern)) {
    adaptationSets.push(parseAdaptationSet((match[1] ?? "") + ">" + (match[2] ?? "")));
  }
  return { id, start, duration, adaptationSets };
}

export function parse(content: string, _baseUrl?: string): DashManifest {
  const mpdMatch = /<MPD\b([^>]*)>/.exec(content);
  const mpdAttrs = mpdMatch?.[1] ?? "";
  const type = attribute(mpdAttrs, "type") === "dynamic" ? "dynamic" : "static";
  const duration = attribute(mpdAttrs, "mediaPresentationDuration");
  const minBufferTime = attribute(mpdAttrs, "minBufferTime");

  const periodPattern = /<Period\b([^>]*)>([\s\S]*?)<\/Period>/g;
  const periods: DashPeriod[] = [];
  for (const match of collect(content, periodPattern)) {
    periods.push(parsePeriod((match[1] ?? "") + ">" + (match[2] ?? "")));
  }
  return { type, duration, minBufferTime, periods };
}
