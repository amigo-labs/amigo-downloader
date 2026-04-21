export interface HlsResolution {
  readonly width: number;
  readonly height: number;
}

export interface HlsVariant {
  readonly url: string;
  readonly bandwidth: number;
  readonly averageBandwidth: number | null;
  readonly resolution: HlsResolution | null;
  readonly codecs: readonly string[];
  readonly frameRate: number | null;
  readonly audioGroup: string | null;
  readonly subtitleGroup: string | null;
  readonly videoGroup: string | null;
}

export interface HlsAudioTrack {
  readonly groupId: string;
  readonly name: string;
  readonly language: string | null;
  readonly uri: string | null;
  readonly default: boolean;
  readonly autoselect: boolean;
  readonly channels: string | null;
}

export interface HlsSubtitleTrack {
  readonly groupId: string;
  readonly name: string;
  readonly language: string | null;
  readonly uri: string | null;
  readonly default: boolean;
  readonly forced: boolean;
}

export interface HlsMasterPlaylist {
  readonly variants: readonly HlsVariant[];
  readonly audioTracks: readonly HlsAudioTrack[];
  readonly subtitleTracks: readonly HlsSubtitleTrack[];
  readonly independentSegments: boolean;
  readonly version: number | null;
}

export interface HlsKey {
  readonly method: string;
  readonly uri: string | null;
  readonly iv: string | null;
}

export interface HlsSegment {
  readonly url: string;
  readonly durationSeconds: number;
  readonly title: string | null;
  readonly byteRange: string | null;
  readonly discontinuity: boolean;
  readonly key: HlsKey | null;
}

export interface HlsMediaPlaylist {
  readonly targetDuration: number | null;
  readonly sequence: number;
  readonly endList: boolean;
  readonly segments: readonly HlsSegment[];
  readonly version: number | null;
}

function parseAttributes(input: string): Record<string, string> {
  const result: Record<string, string> = {};
  const pattern = /([A-Z0-9-]+)=("([^"]*)"|[^,]*)/g;
  for (const match of input.matchAll(pattern)) {
    const key = match[1]!;
    const raw = match[3] ?? match[2] ?? "";
    result[key] = raw;
  }
  return result;
}

function resolve(baseUrl: string | undefined, uri: string): string {
  if (!baseUrl) {
    return uri;
  }
  try {
    return new URL(uri, baseUrl).toString();
  } catch {
    return uri;
  }
}

function splitCodecs(raw: string | undefined): string[] {
  if (!raw) {
    return [];
  }
  return raw.split(",").map((item) => item.trim()).filter((item) => item.length > 0);
}

function parseResolution(raw: string | undefined): HlsResolution | null {
  if (!raw) {
    return null;
  }
  const match = /^(\d+)x(\d+)$/.exec(raw.trim());
  if (!match) {
    return null;
  }
  return { width: Number.parseInt(match[1]!, 10), height: Number.parseInt(match[2]!, 10) };
}

function toBoolean(raw: string | undefined): boolean {
  return raw === "YES";
}

export function parseMaster(content: string, baseUrl?: string): HlsMasterPlaylist {
  const lines = content.split(/\r?\n/);
  const variants: HlsVariant[] = [];
  const audioTracks: HlsAudioTrack[] = [];
  const subtitleTracks: HlsSubtitleTrack[] = [];
  let independentSegments = false;
  let version: number | null = null;

  let pendingVariantAttrs: Record<string, string> | null = null;

  for (const raw of lines) {
    const line = raw.trim();
    if (line.length === 0) {
      continue;
    }
    if (line === "#EXT-X-INDEPENDENT-SEGMENTS") {
      independentSegments = true;
      continue;
    }
    if (line.startsWith("#EXT-X-VERSION:")) {
      version = Number.parseInt(line.slice("#EXT-X-VERSION:".length), 10);
      continue;
    }
    if (line.startsWith("#EXT-X-STREAM-INF:")) {
      pendingVariantAttrs = parseAttributes(line.slice("#EXT-X-STREAM-INF:".length));
      continue;
    }
    if (line.startsWith("#EXT-X-MEDIA:")) {
      const attrs = parseAttributes(line.slice("#EXT-X-MEDIA:".length));
      const type = attrs["TYPE"];
      const common = {
        groupId: attrs["GROUP-ID"] ?? "",
        name: attrs["NAME"] ?? "",
        language: attrs["LANGUAGE"] ?? null,
        uri: attrs["URI"] ? resolve(baseUrl, attrs["URI"]) : null,
      };
      if (type === "AUDIO") {
        audioTracks.push({
          ...common,
          default: toBoolean(attrs["DEFAULT"]),
          autoselect: toBoolean(attrs["AUTOSELECT"]),
          channels: attrs["CHANNELS"] ?? null,
        });
      } else if (type === "SUBTITLES") {
        subtitleTracks.push({
          ...common,
          default: toBoolean(attrs["DEFAULT"]),
          forced: toBoolean(attrs["FORCED"]),
        });
      }
      continue;
    }
    if (line.startsWith("#")) {
      continue;
    }
    if (pendingVariantAttrs) {
      const attrs = pendingVariantAttrs;
      pendingVariantAttrs = null;
      const bandwidth = Number.parseInt(attrs["BANDWIDTH"] ?? "0", 10) || 0;
      const averageBandwidth = attrs["AVERAGE-BANDWIDTH"]
        ? Number.parseInt(attrs["AVERAGE-BANDWIDTH"], 10)
        : null;
      const frameRate = attrs["FRAME-RATE"] ? Number.parseFloat(attrs["FRAME-RATE"]) : null;
      variants.push({
        url: resolve(baseUrl, line),
        bandwidth,
        averageBandwidth,
        resolution: parseResolution(attrs["RESOLUTION"]),
        codecs: splitCodecs(attrs["CODECS"]),
        frameRate,
        audioGroup: attrs["AUDIO"] ?? null,
        subtitleGroup: attrs["SUBTITLES"] ?? null,
        videoGroup: attrs["VIDEO"] ?? null,
      });
    }
  }

  return { variants, audioTracks, subtitleTracks, independentSegments, version };
}

export function parseMedia(content: string, baseUrl?: string): HlsMediaPlaylist {
  const lines = content.split(/\r?\n/);
  const segments: HlsSegment[] = [];
  let targetDuration: number | null = null;
  let sequence = 0;
  let endList = false;
  let version: number | null = null;
  let pendingDuration: number | null = null;
  let pendingTitle: string | null = null;
  let pendingByteRange: string | null = null;
  let pendingDiscontinuity = false;
  let currentKey: HlsKey | null = null;

  for (const raw of lines) {
    const line = raw.trim();
    if (line.length === 0) {
      continue;
    }
    if (line.startsWith("#EXT-X-VERSION:")) {
      version = Number.parseInt(line.slice("#EXT-X-VERSION:".length), 10);
      continue;
    }
    if (line.startsWith("#EXT-X-TARGETDURATION:")) {
      targetDuration = Number.parseInt(line.slice("#EXT-X-TARGETDURATION:".length), 10);
      continue;
    }
    if (line.startsWith("#EXT-X-MEDIA-SEQUENCE:")) {
      sequence = Number.parseInt(line.slice("#EXT-X-MEDIA-SEQUENCE:".length), 10);
      continue;
    }
    if (line === "#EXT-X-ENDLIST") {
      endList = true;
      continue;
    }
    if (line === "#EXT-X-DISCONTINUITY") {
      pendingDiscontinuity = true;
      continue;
    }
    if (line.startsWith("#EXT-X-KEY:")) {
      const attrs = parseAttributes(line.slice("#EXT-X-KEY:".length));
      currentKey = {
        method: attrs["METHOD"] ?? "NONE",
        uri: attrs["URI"] ? resolve(baseUrl, attrs["URI"]) : null,
        iv: attrs["IV"] ?? null,
      };
      continue;
    }
    if (line.startsWith("#EXT-X-BYTERANGE:")) {
      pendingByteRange = line.slice("#EXT-X-BYTERANGE:".length);
      continue;
    }
    if (line.startsWith("#EXTINF:")) {
      const body = line.slice("#EXTINF:".length);
      const comma = body.indexOf(",");
      const durationPart = comma >= 0 ? body.slice(0, comma) : body;
      const titlePart = comma >= 0 ? body.slice(comma + 1) : "";
      pendingDuration = Number.parseFloat(durationPart);
      pendingTitle = titlePart.length > 0 ? titlePart : null;
      continue;
    }
    if (line.startsWith("#")) {
      continue;
    }
    if (pendingDuration !== null) {
      segments.push({
        url: resolve(baseUrl, line),
        durationSeconds: pendingDuration,
        title: pendingTitle,
        byteRange: pendingByteRange,
        discontinuity: pendingDiscontinuity,
        key: currentKey,
      });
      pendingDuration = null;
      pendingTitle = null;
      pendingByteRange = null;
      pendingDiscontinuity = false;
    }
  }

  return { targetDuration, sequence, endList, segments, version };
}
