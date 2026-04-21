const UNIT_MS: Record<string, number> = {
  ms: 1,
  milliseconds: 1,
  s: 1000,
  sec: 1000,
  secs: 1000,
  second: 1000,
  seconds: 1000,
  m: 60_000,
  min: 60_000,
  mins: 60_000,
  minute: 60_000,
  minutes: 60_000,
  h: 3_600_000,
  hr: 3_600_000,
  hour: 3_600_000,
  hours: 3_600_000,
  d: 86_400_000,
  day: 86_400_000,
  days: 86_400_000,
};

function parseClockNotation(input: string): number | null {
  const parts = input.split(":");
  if (parts.length < 2 || parts.length > 3) {
    return null;
  }
  let seconds = 0;
  for (const part of parts) {
    const value = Number.parseFloat(part);
    if (!Number.isFinite(value)) {
      return null;
    }
    seconds = seconds * 60 + value;
  }
  return Math.round(seconds * 1000);
}

export function parseDuration(input: string): number | null {
  const trimmed = input.trim();
  if (trimmed.length === 0) {
    return null;
  }
  if (trimmed.includes(":")) {
    return parseClockNotation(trimmed);
  }
  let total = 0;
  let matched = false;
  const pattern = /([0-9]+(?:\.[0-9]+)?)\s*([A-Za-z]+)/g;
  for (const match of trimmed.matchAll(pattern)) {
    const quantity = Number.parseFloat(match[1]!);
    const unit = match[2]!.toLowerCase();
    const factor = UNIT_MS[unit];
    if (factor === undefined || !Number.isFinite(quantity)) {
      return null;
    }
    total += quantity * factor;
    matched = true;
  }
  if (!matched) {
    return null;
  }
  return Math.round(total);
}
