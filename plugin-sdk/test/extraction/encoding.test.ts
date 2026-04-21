import { describe, expect, it } from "vitest";
import {
  base64Decode,
  base64DecodeToString,
  base64Encode,
  hexDecode,
  hexEncode,
  htmlDecode,
  htmlEncode,
  unicodeDecode,
  urlDecode,
  urlEncode,
} from "../../src/extraction/encoding.js";

describe("htmlDecode", () => {
  it("decodes named, decimal, and hex entities", () => {
    expect(htmlDecode("&amp;&lt;&gt;&quot;")).toBe('&<>"');
    expect(htmlDecode("&#64;&#x41;")).toBe("@A");
    expect(htmlDecode("&nbsp;")).toBe(" ");
    expect(htmlDecode("&#x1F600;")).toBe("😀");
  });

  it("leaves unknown entities untouched", () => {
    expect(htmlDecode("&unknown;")).toBe("&unknown;");
  });
});

describe("htmlEncode", () => {
  it("encodes unsafe chars", () => {
    expect(htmlEncode(`<a href="x&y">'</a>`)).toBe(
      "&lt;a href=&quot;x&amp;y&quot;&gt;&#39;&lt;/a&gt;",
    );
  });
});

describe("unicodeDecode", () => {
  it("handles \\uXXXX, \\u{...}, \\xXX", () => {
    expect(unicodeDecode("\\u0041\\x42\\u{1F600}")).toBe("AB😀");
  });
});

describe("url encoding", () => {
  it("roundtrips", () => {
    expect(urlDecode(urlEncode("a b c"))).toBe("a b c");
  });
});

describe("base64", () => {
  it("roundtrips strings", () => {
    expect(base64DecodeToString(base64Encode("hello"))).toBe("hello");
  });

  it("roundtrips binary", () => {
    const bytes = new Uint8Array([0, 1, 2, 3, 250, 251, 252]);
    const encoded = base64Encode(bytes);
    expect(Array.from(base64Decode(encoded))).toEqual(Array.from(bytes));
  });

  it("produces canonical padding for short inputs", () => {
    expect(base64Encode("f")).toBe("Zg==");
    expect(base64Encode("fo")).toBe("Zm8=");
    expect(base64Encode("foo")).toBe("Zm9v");
  });

  it("tolerates whitespace and missing padding on decode", () => {
    expect(new TextDecoder().decode(base64Decode("Zm9v\nYg=="))).toBe("foob");
    expect(new TextDecoder().decode(base64Decode("Zm9v"))).toBe("foo");
  });
});

describe("hex", () => {
  it("roundtrips", () => {
    const bytes = new Uint8Array([0xde, 0xad, 0xbe, 0xef]);
    expect(hexEncode(bytes)).toBe("deadbeef");
    expect(Array.from(hexDecode("deadbeef"))).toEqual([0xde, 0xad, 0xbe, 0xef]);
  });

  it("throws on odd length", () => {
    expect(() => hexDecode("abc")).toThrow(/odd-length/);
  });
});
