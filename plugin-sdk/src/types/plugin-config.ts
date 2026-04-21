export type PluginConfigValue = string | number | boolean | null;

export interface PluginConfig {
  get<T extends PluginConfigValue>(key: string, fallback: T): T;
  getString(key: string, fallback?: string | null): string | null;
  getNumber(key: string, fallback?: number | null): number | null;
  getBoolean(key: string, fallback?: boolean | null): boolean | null;
  has(key: string): boolean;
  keys(): readonly string[];
  snapshot(): Readonly<Record<string, PluginConfigValue>>;
}

function hasKey(
  values: Readonly<Record<string, PluginConfigValue>>,
  key: string,
): boolean {
  return Object.prototype.hasOwnProperty.call(values, key);
}

export function pluginConfig(
  values: Readonly<Record<string, PluginConfigValue>>,
): PluginConfig {
  return {
    get<T extends PluginConfigValue>(key: string, fallback: T): T {
      if (!hasKey(values, key)) {
        return fallback;
      }
      return values[key] as T;
    },
    getString: (key, fallback = null) => {
      if (!hasKey(values, key)) {
        return fallback;
      }
      const value = values[key];
      return typeof value === "string" ? value : fallback;
    },
    getNumber: (key, fallback = null) => {
      if (!hasKey(values, key)) {
        return fallback;
      }
      const value = values[key];
      return typeof value === "number" && Number.isFinite(value) ? value : fallback;
    },
    getBoolean: (key, fallback = null) => {
      if (!hasKey(values, key)) {
        return fallback;
      }
      const value = values[key];
      return typeof value === "boolean" ? value : fallback;
    },
    has: (key) => hasKey(values, key),
    keys: () => Object.keys(values),
    snapshot: () => ({ ...values }),
  };
}
