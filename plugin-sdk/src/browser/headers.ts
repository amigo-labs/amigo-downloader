export class Headers {
  private readonly entries = new Map<string, { name: string; value: string }>();

  constructor(initial?: Readonly<Record<string, string>> | Headers) {
    if (!initial) {
      return;
    }
    if (initial instanceof Headers) {
      for (const [key, entry] of initial.entries) {
        this.entries.set(key, { name: entry.name, value: entry.value });
      }
      return;
    }
    for (const [name, value] of Object.entries(initial)) {
      this.set(name, value);
    }
  }

  set(name: string, value: string): void {
    this.entries.set(name.toLowerCase(), { name, value });
  }

  get(name: string): string | null {
    return this.entries.get(name.toLowerCase())?.value ?? null;
  }

  has(name: string): boolean {
    return this.entries.has(name.toLowerCase());
  }

  delete(name: string): void {
    this.entries.delete(name.toLowerCase());
  }

  clear(): void {
    this.entries.clear();
  }

  setAll(values: Readonly<Record<string, string>>): void {
    for (const [name, value] of Object.entries(values)) {
      this.set(name, value);
    }
  }

  clone(): Headers {
    return new Headers(this);
  }

  toRecord(): Record<string, string> {
    const record: Record<string, string> = {};
    for (const entry of this.entries.values()) {
      record[entry.name] = entry.value;
    }
    return record;
  }

  *[Symbol.iterator](): IterableIterator<readonly [string, string]> {
    for (const entry of this.entries.values()) {
      yield [entry.name, entry.value] as const;
    }
  }

  get size(): number {
    return this.entries.size;
  }
}
