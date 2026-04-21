export type UrlPattern = RegExp | string;

function escapeRegexChar(char: string): string {
  return char.replace(/[.+^${}()|[\]\\]/g, "\\$&");
}

export function compilePattern(pattern: UrlPattern): RegExp {
  if (pattern instanceof RegExp) {
    return pattern;
  }
  let source = "^";
  for (const char of pattern) {
    if (char === "*") {
      source += ".*";
    } else if (char === "?") {
      source += ".";
    } else {
      source += escapeRegexChar(char);
    }
  }
  source += "$";
  return new RegExp(source, "i");
}

export function matchesAny(patterns: readonly UrlPattern[], url: string): boolean {
  for (const pattern of patterns) {
    if (compilePattern(pattern).test(url)) {
      return true;
    }
  }
  return false;
}
