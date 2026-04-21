export interface ParsedArgs {
  readonly positional: readonly string[];
  readonly flags: Readonly<Record<string, string | true>>;
}

export function parseArgs(argv: readonly string[]): ParsedArgs {
  const positional: string[] = [];
  const flags: Record<string, string | true> = {};
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index]!;
    if (token.startsWith("--")) {
      const body = token.slice(2);
      const equalsIndex = body.indexOf("=");
      if (equalsIndex >= 0) {
        flags[body.slice(0, equalsIndex)] = body.slice(equalsIndex + 1);
      } else {
        const next = argv[index + 1];
        if (next !== undefined && !next.startsWith("-")) {
          flags[body] = next;
          index += 1;
        } else {
          flags[body] = true;
        }
      }
      continue;
    }
    if (token.startsWith("-") && token.length > 1) {
      flags[token.slice(1)] = true;
      continue;
    }
    positional.push(token);
  }
  return { positional, flags };
}
