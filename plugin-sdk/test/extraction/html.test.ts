import { describe, expect, it } from "vitest";
import { stripTags } from "../../src/extraction/html.js";

describe("stripTags", () => {
  it("removes tags but keeps text", () => {
    expect(stripTags("<p>hello <b>world</b></p>")).toBe("hello world");
  });

  it("strips scripts and styles with contents", () => {
    expect(stripTags("<style>.a{}</style>Foo<script>alert(1)</script>")).toBe("Foo");
  });

  it("collapses whitespace", () => {
    expect(stripTags("<div>a   <span>b</span>\n\nc</div>")).toBe("a b c");
  });
});
