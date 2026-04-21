import { describe, expect, it } from "vitest";
import { dash } from "../../src/media/index.js";

const MPD = `<?xml version="1.0"?>
<MPD type="static" mediaPresentationDuration="PT1M2.5S" minBufferTime="PT2S">
  <Period id="0" start="PT0S" duration="PT1M2.5S">
    <AdaptationSet id="v" contentType="video" mimeType="video/mp4">
      <Representation id="360p" bandwidth="800000" width="640" height="360" codecs="avc1.42e00a">
        <BaseURL>v-360p.mp4</BaseURL>
      </Representation>
      <Representation id="720p" bandwidth="1800000" width="1280" height="720" codecs="avc1.4d001f">
        <BaseURL>v-720p.mp4</BaseURL>
      </Representation>
    </AdaptationSet>
    <AdaptationSet id="a" contentType="audio" mimeType="audio/mp4" lang="en">
      <Representation id="aac" bandwidth="128000" codecs="mp4a.40.2"/>
    </AdaptationSet>
  </Period>
</MPD>
`;

describe("DASH manifest", () => {
  it("parses MPD attributes, periods, adaptation sets", () => {
    const manifest = dash.parse(MPD);
    expect(manifest.type).toBe("static");
    expect(manifest.duration).toBe("PT1M2.5S");
    expect(manifest.periods).toHaveLength(1);
    const period = manifest.periods[0]!;
    expect(period.adaptationSets).toHaveLength(2);

    const video = period.adaptationSets[0]!;
    expect(video.contentType).toBe("video");
    expect(video.representations).toHaveLength(2);
    expect(video.representations[1]?.bandwidth).toBe(1800000);
    expect(video.representations[1]?.height).toBe(720);
    expect(video.representations[1]?.baseUrl).toBe("v-720p.mp4");

    const audio = period.adaptationSets[1]!;
    expect(audio.contentType).toBe("audio");
    expect(audio.language).toBe("en");
    expect(audio.representations[0]?.codecs).toBe("mp4a.40.2");
  });
});
