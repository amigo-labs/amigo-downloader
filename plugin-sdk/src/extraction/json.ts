export function parse<T = unknown>(source: string): T {
  return JSON.parse(source) as T;
}

function splitPath(path: string): string[] {
  return path.split("/").filter((segment) => segment.length > 0);
}

export function walk(value: unknown, path: string): unknown {
  const segments = splitPath(path);
  let current: unknown = value;
  for (const segment of segments) {
    if (current === null || current === undefined) {
      return null;
    }
    if (Array.isArray(current)) {
      const index = Number.parseInt(segment, 10);
      if (Number.isNaN(index) || index < 0 || index >= current.length) {
        return null;
      }
      current = current[index];
      continue;
    }
    if (typeof current === "object") {
      current = (current as Record<string, unknown>)[segment];
      continue;
    }
    return null;
  }
  return current ?? null;
}

export function getString(value: unknown, path: string): string | null {
  const target = walk(value, path);
  return typeof target === "string" ? target : null;
}

export function getNumber(value: unknown, path: string): number | null {
  const target = walk(value, path);
  return typeof target === "number" && Number.isFinite(target) ? target : null;
}

export function getBoolean(value: unknown, path: string): boolean | null {
  const target = walk(value, path);
  return typeof target === "boolean" ? target : null;
}

export function getArray<T = unknown>(value: unknown, path: string): T[] | null {
  const target = walk(value, path);
  return Array.isArray(target) ? (target as T[]) : null;
}

export function getObject(value: unknown, path: string): Record<string, unknown> | null {
  const target = walk(value, path);
  if (target && typeof target === "object" && !Array.isArray(target)) {
    return target as Record<string, unknown>;
  }
  return null;
}

function unescapeJsonLiteral(input: string): string {
  return input.replace(/\\(u[0-9a-fA-F]{4}|x[0-9a-fA-F]{2}|[\\"'nrtbf/])/g, (_, sequence: string) => {
    if (sequence.startsWith("u")) {
      return String.fromCharCode(Number.parseInt(sequence.slice(1), 16));
    }
    if (sequence.startsWith("x")) {
      return String.fromCharCode(Number.parseInt(sequence.slice(1), 16));
    }
    switch (sequence) {
      case "n":
        return "\n";
      case "r":
        return "\r";
      case "t":
        return "\t";
      case "b":
        return "\b";
      case "f":
        return "\f";
      case '"':
      case "'":
      case "\\":
      case "/":
        return sequence;
      default:
        return sequence;
    }
  });
}

export function extract(source: string, key: string): string | null {
  const escapedKey = key.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const patterns: RegExp[] = [
    new RegExp(`"${escapedKey}"\\s*:\\s*"((?:[^"\\\\]|\\\\.)*)"`),
    new RegExp(`'${escapedKey}'\\s*:\\s*'((?:[^'\\\\]|\\\\.)*)'`),
    new RegExp(`\\b${escapedKey}\\s*:\\s*"((?:[^"\\\\]|\\\\.)*)"`),
    new RegExp(`\\b${escapedKey}\\s*:\\s*'((?:[^'\\\\]|\\\\.)*)'`),
    new RegExp(`"${escapedKey}"\\s*:\\s*([0-9]+(?:\\.[0-9]+)?|true|false|null)`),
  ];
  for (const pattern of patterns) {
    const match = pattern.exec(source);
    if (match && match[1] !== undefined) {
      return unescapeJsonLiteral(match[1]);
    }
  }
  return null;
}
