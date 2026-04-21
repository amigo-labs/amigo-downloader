const UNIT_FACTORS: Record<string, number> = {
  b: 1,
  byte: 1,
  bytes: 1,
  kb: 1000,
  kib: 1024,
  mb: 1000 ** 2,
  mib: 1024 ** 2,
  gb: 1000 ** 3,
  gib: 1024 ** 3,
  tb: 1000 ** 4,
  tib: 1024 ** 4,
  pb: 1000 ** 5,
  pib: 1024 ** 5,
};

export function parseSize(input: string): number | null {
  const trimmed = input.trim().replace(/,/g, "");
  if (trimmed.length === 0) {
    return null;
  }
  const match = /^([0-9]+(?:\.[0-9]+)?)\s*([A-Za-z]+)?$/.exec(trimmed);
  if (!match) {
    return null;
  }
  const quantity = Number.parseFloat(match[1]!);
  if (!Number.isFinite(quantity)) {
    return null;
  }
  const rawUnit = (match[2] ?? "b").toLowerCase();
  const factor = UNIT_FACTORS[rawUnit];
  if (factor === undefined) {
    return null;
  }
  return Math.round(quantity * factor);
}
