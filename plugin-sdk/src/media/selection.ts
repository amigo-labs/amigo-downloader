export interface SelectableVariant {
  readonly bandwidth: number;
  readonly width?: number | null;
  readonly height?: number | null;
  readonly codecs?: readonly string[] | string | null;
}

export interface SelectionCriteria {
  readonly maxHeight?: number;
  readonly minHeight?: number;
  readonly maxBandwidth?: number;
  readonly minBandwidth?: number;
  readonly preferCodec?: string | RegExp;
}

function codecList(codecs: SelectableVariant["codecs"]): string[] {
  if (!codecs) {
    return [];
  }
  if (typeof codecs === "string") {
    return codecs.split(",").map((c) => c.trim()).filter((c) => c.length > 0);
  }
  return codecs.slice();
}

function matchesCodec(codecs: string[], preference: string | RegExp): boolean {
  for (const codec of codecs) {
    if (preference instanceof RegExp) {
      if (preference.test(codec)) {
        return true;
      }
    } else if (codec.includes(preference)) {
      return true;
    }
  }
  return false;
}

function applyCriteria<T extends SelectableVariant>(
  variants: readonly T[],
  criteria: SelectionCriteria | undefined,
): T[] {
  if (!criteria) {
    return variants.slice();
  }
  return variants.filter((variant) => {
    if (criteria.maxHeight !== undefined && variant.height !== null && variant.height !== undefined) {
      if (variant.height > criteria.maxHeight) {
        return false;
      }
    }
    if (criteria.minHeight !== undefined && variant.height !== null && variant.height !== undefined) {
      if (variant.height < criteria.minHeight) {
        return false;
      }
    }
    if (criteria.maxBandwidth !== undefined && variant.bandwidth > criteria.maxBandwidth) {
      return false;
    }
    if (criteria.minBandwidth !== undefined && variant.bandwidth < criteria.minBandwidth) {
      return false;
    }
    return true;
  });
}

export function selectBestVariant<T extends SelectableVariant>(
  variants: readonly T[],
  criteria?: SelectionCriteria,
): T | null {
  const pool = applyCriteria(variants, criteria);
  if (pool.length === 0) {
    return null;
  }
  if (criteria?.preferCodec) {
    const preferred = pool.filter((variant) => matchesCodec(codecList(variant.codecs), criteria.preferCodec!));
    if (preferred.length > 0) {
      return preferred.reduce((best, current) => (current.bandwidth > best.bandwidth ? current : best));
    }
  }
  return pool.reduce((best, current) => (current.bandwidth > best.bandwidth ? current : best));
}

export function selectWorstVariant<T extends SelectableVariant>(
  variants: readonly T[],
  criteria?: SelectionCriteria,
): T | null {
  const pool = applyCriteria(variants, criteria);
  if (pool.length === 0) {
    return null;
  }
  return pool.reduce((best, current) => (current.bandwidth < best.bandwidth ? current : best));
}

export function filterByResolution<T extends SelectableVariant>(
  variants: readonly T[],
  bounds: { min?: number; max?: number },
): T[] {
  return variants.filter((variant) => {
    const height = variant.height ?? null;
    if (height === null) {
      return false;
    }
    if (bounds.min !== undefined && height < bounds.min) {
      return false;
    }
    if (bounds.max !== undefined && height > bounds.max) {
      return false;
    }
    return true;
  });
}

export function filterByCodec<T extends SelectableVariant>(
  variants: readonly T[],
  pattern: RegExp,
): T[] {
  return variants.filter((variant) => matchesCodec(codecList(variant.codecs), pattern));
}
