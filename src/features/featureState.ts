// Pure switch lookups shared by the UI, shell preferences and tests: no React,
// no Tauri, so Node tests can import them directly.
import defaults from "./featureDefaults.json";

export type FeatureKey = keyof typeof defaults;
export type FeatureSettings = Record<string, { enabled: boolean; config: any } | undefined>;

export const FEATURE_DEFAULTS: Readonly<Record<FeatureKey, boolean>> = defaults;
export const FEATURE_KEYS = Object.keys(defaults) as FeatureKey[];

export function isFeatureOn(settings: FeatureSettings, key: FeatureKey): boolean {
  return settings[key]?.enabled ?? FEATURE_DEFAULTS[key];
}

/** The switch's saved config merged over `fallback`; bad or missing config gives `fallback`. */
export function featureConfig<T extends object>(settings: FeatureSettings, key: FeatureKey, fallback: T): T {
  const saved = settings[key]?.config;
  return saved && typeof saved === "object" && !Array.isArray(saved) ? { ...fallback, ...saved } : { ...fallback };
}

