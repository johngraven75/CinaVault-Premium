// Feature switches from Advanced > Feature Matrix. Defaults live in
// featureDefaults.json, which the Rust side reads too (feature_flags.rs), so
// front end and back end agree on a switch nobody has touched yet.
import { invoke } from "@tauri-apps/api/core";

import { useAppStore } from "../store/appStore";
import {
  FEATURE_DEFAULTS,
  FEATURE_KEYS,
  featureConfig,
  isFeatureOn,
  type FeatureKey,
  type FeatureSettings,
} from "./featureState.ts";

export { FEATURE_DEFAULTS, FEATURE_KEYS, featureConfig, isFeatureOn };
export type { FeatureKey, FeatureSettings };

/** Persists a switch (and its config) to the back end, then to the UI store. */
export async function saveFeature(key: FeatureKey, enabled: boolean, config: object = {}): Promise<void> {
  await invoke("set_feature_setting", { key, enabled, config: JSON.stringify(config) });
  const { featureSettings, setFeatureSettings } = useAppStore.getState();
  setFeatureSettings({ ...featureSettings, [key]: { enabled, config } });
}

/** React access to one switch: whether it is on and its config. */
export function useFeature<T extends object>(key: FeatureKey, fallback: T = {} as T) {
  const settings = useAppStore((state) => state.featureSettings);
  const enabled = isFeatureOn(settings, key);
  const config = featureConfig(settings, key, fallback);
  return {
    enabled,
    config,
    setEnabled: (next: boolean) => saveFeature(key, next, settings[key]?.config ?? {}),
    setConfig: (next: Partial<T>) => saveFeature(key, enabled, { ...config, ...next }),
  };
}

/**
 * Loads saved switches from the back end at startup. For every matrix switch
 * the saved row wins, else its default; values cached in local settings from
 * older builds (which never reached the back end) are dropped so the UI and
 * the Rust side always agree.
 */
export async function syncFeatureSettingsFromBackend(): Promise<void> {
  const rows = await invoke<Array<{ key: string; enabled: boolean; config: unknown }>>("get_feature_settings");
  const saved = new Map(rows.map((row) => [row.key, row]));
  const { featureSettings, setFeatureSettings } = useAppStore.getState();
  const next: FeatureSettings = { ...featureSettings };
  for (const key of FEATURE_KEYS) {
    const row = saved.get(key);
    next[key] = row
      ? { enabled: Boolean(row.enabled), config: row.config && typeof row.config === "object" ? row.config : {} }
      : { enabled: FEATURE_DEFAULTS[key], config: {} };
  }
  setFeatureSettings(next as Record<string, { enabled: boolean; config: any }>);
}
