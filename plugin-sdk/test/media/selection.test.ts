import { describe, expect, it } from "vitest";
import {
  filterByCodec,
  filterByResolution,
  selectBestVariant,
  selectWorstVariant,
} from "../../src/media/index.js";

const VARIANTS = [
  { bandwidth: 500_000, width: 320, height: 180, codecs: ["avc1.42e01e"] },
  { bandwidth: 2_000_000, width: 1280, height: 720, codecs: ["avc1.4d001f"] },
  { bandwidth: 5_000_000, width: 1920, height: 1080, codecs: ["avc1.640028"] },
  { bandwidth: 4_000_000, width: 1920, height: 1080, codecs: ["vp9"] },
];

describe("selectBestVariant", () => {
  it("picks highest bandwidth by default", () => {
    expect(selectBestVariant(VARIANTS)?.bandwidth).toBe(5_000_000);
  });

  it("respects maxHeight", () => {
    expect(selectBestVariant(VARIANTS, { maxHeight: 720 })?.bandwidth).toBe(2_000_000);
  });

  it("prefers codec when available", () => {
    expect(selectBestVariant(VARIANTS, { preferCodec: /vp9/ })?.bandwidth).toBe(4_000_000);
  });
});

describe("selectWorstVariant", () => {
  it("picks lowest bandwidth", () => {
    expect(selectWorstVariant(VARIANTS)?.bandwidth).toBe(500_000);
  });
});

describe("filters", () => {
  it("filterByResolution keeps in-range entries", () => {
    expect(filterByResolution(VARIANTS, { max: 720 })).toHaveLength(2);
  });

  it("filterByCodec matches against codec list", () => {
    expect(filterByCodec(VARIANTS, /vp9/)).toHaveLength(1);
  });
});
