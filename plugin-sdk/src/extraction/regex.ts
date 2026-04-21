export interface RegexResult {
  matches(): boolean;
  getMatch(groupIndex?: number): string | null;
  getMatches(): string[][];
  getColumn(index: number): string[];
}

function toRegExp(pattern: string | RegExp, flagsFallback: string): RegExp {
  if (pattern instanceof RegExp) {
    return pattern.flags.includes("g") ? pattern : new RegExp(pattern.source, `${pattern.flags}g`);
  }
  return new RegExp(pattern, flagsFallback.includes("g") ? flagsFallback : `${flagsFallback}g`);
}

export function regex(source: string, pattern: string | RegExp, flags = "g"): RegexResult {
  const compiled = toRegExp(pattern, flags);
  const rows: string[][] = [];
  for (const match of source.matchAll(compiled)) {
    rows.push(Array.from(match));
  }

  return {
    matches: () => rows.length > 0,
    getMatch: (groupIndex = 0) => {
      if (rows.length === 0) {
        return null;
      }
      const first = rows[0]!;
      if (groupIndex < 0 || groupIndex >= first.length) {
        return null;
      }
      return first[groupIndex] ?? null;
    },
    getMatches: () => rows.map((row) => [...row]),
    getColumn: (index) =>
      rows
        .map((row) => row[index])
        .filter((value): value is string => value !== undefined),
  };
}

export function sourceContains(source: string, pattern: string | RegExp): boolean {
  if (pattern instanceof RegExp) {
    return pattern.test(source);
  }
  return source.includes(pattern);
}

export { regex as matchRegex };
