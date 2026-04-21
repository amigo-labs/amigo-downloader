import { parseDuration } from "./parse-duration.js";

function parseRelative(input: string, now: number): number | null {
  const trimmed = input.trim().toLowerCase();
  if (trimmed === "now") {
    return now;
  }
  if (trimmed === "yesterday") {
    return now - 86_400_000;
  }
  if (trimmed === "tomorrow") {
    return now + 86_400_000;
  }
  const agoMatch = /^(.*?)\s+ago$/.exec(trimmed);
  if (agoMatch) {
    const duration = parseDuration(agoMatch[1]!);
    if (duration !== null) {
      return now - duration;
    }
  }
  const inMatch = /^in\s+(.*)$/.exec(trimmed);
  if (inMatch) {
    const duration = parseDuration(inMatch[1]!);
    if (duration !== null) {
      return now + duration;
    }
  }
  return null;
}

export interface ParseDateOptions {
  readonly now?: number;
}

export function parseDate(input: string, options: ParseDateOptions = {}): Date | null {
  const now = options.now ?? Date.now();
  const relative = parseRelative(input, now);
  if (relative !== null) {
    return new Date(relative);
  }
  const timestamp = Date.parse(input);
  if (!Number.isNaN(timestamp)) {
    return new Date(timestamp);
  }
  return null;
}
