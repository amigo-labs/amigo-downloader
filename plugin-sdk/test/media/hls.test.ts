import { describe, expect, it } from "vitest";
import { hls } from "../../src/media/index.js";

const MASTER = `#EXTM3U
#EXT-X-VERSION:4
#EXT-X-INDEPENDENT-SEGMENTS
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="aac",NAME="English",DEFAULT=YES,LANGUAGE="en",URI="audio-en.m3u8"
#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID="subs",NAME="English",LANGUAGE="en",URI="subs-en.m3u8",FORCED=NO
#EXT-X-STREAM-INF:BANDWIDTH=800000,RESOLUTION=640x360,CODECS="avc1.42e00a,mp4a.40.2",AUDIO="aac",FRAME-RATE=30.000
low/index.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=1800000,RESOLUTION=1280x720,CODECS="avc1.4d001f,mp4a.40.2",AUDIO="aac",FRAME-RATE=30.000
mid/index.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=4500000,RESOLUTION=1920x1080,CODECS="avc1.640028,mp4a.40.2",AUDIO="aac",FRAME-RATE=30.000
high/index.m3u8
`;

describe("HLS master playlist", () => {
  it("parses variants with bandwidth, resolution, codecs", () => {
    const master = hls.parseMaster(MASTER, "https://cdn.test/stream/");
    expect(master.variants).toHaveLength(3);
    const [low, mid, high] = master.variants;
    expect(low?.url).toBe("https://cdn.test/stream/low/index.m3u8");
    expect(low?.bandwidth).toBe(800000);
    expect(low?.resolution).toEqual({ width: 640, height: 360 });
    expect(low?.codecs).toEqual(["avc1.42e00a", "mp4a.40.2"]);
    expect(mid?.resolution?.height).toBe(720);
    expect(high?.bandwidth).toBe(4500000);
    expect(master.independentSegments).toBe(true);
    expect(master.version).toBe(4);
  });

  it("parses audio and subtitle media tracks", () => {
    const master = hls.parseMaster(MASTER, "https://cdn.test/stream/");
    expect(master.audioTracks).toHaveLength(1);
    expect(master.audioTracks[0]?.uri).toBe("https://cdn.test/stream/audio-en.m3u8");
    expect(master.audioTracks[0]?.default).toBe(true);
    expect(master.subtitleTracks[0]?.forced).toBe(false);
  });
});

const MEDIA = `#EXTM3U
#EXT-X-VERSION:3
#EXT-X-TARGETDURATION:6
#EXT-X-MEDIA-SEQUENCE:0
#EXT-X-KEY:METHOD=AES-128,URI="key.bin",IV=0x00000000000000000000000000000001
#EXTINF:6.0,
seg-0.ts
#EXTINF:6.0,
seg-1.ts
#EXT-X-ENDLIST
`;

describe("HLS media playlist", () => {
  it("parses segments and picks up the encryption key", () => {
    const media = hls.parseMedia(MEDIA, "https://cdn.test/stream/low/");
    expect(media.segments).toHaveLength(2);
    expect(media.segments[0]?.url).toBe("https://cdn.test/stream/low/seg-0.ts");
    expect(media.segments[0]?.durationSeconds).toBe(6);
    expect(media.segments[0]?.key?.method).toBe("AES-128");
    expect(media.segments[0]?.key?.uri).toBe("https://cdn.test/stream/low/key.bin");
    expect(media.endList).toBe(true);
    expect(media.targetDuration).toBe(6);
  });
});
