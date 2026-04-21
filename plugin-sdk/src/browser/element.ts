import type { HostHtmlElement } from "../host/types.js";

export class Element {
  constructor(private readonly node: HostHtmlElement) {}

  get tag(): string {
    return this.node.tag;
  }

  text(): string {
    return this.node.text;
  }

  html(): string {
    return this.node.html;
  }

  attr(name: string): string | null {
    const key = name.toLowerCase();
    for (const [attribute, value] of Object.entries(this.node.attributes)) {
      if (attribute.toLowerCase() === key) {
        return value;
      }
    }
    return null;
  }

  hasAttr(name: string): boolean {
    return this.attr(name) !== null;
  }

  children(): Element[] {
    return this.node.children.map((child) => new Element(child));
  }
}
