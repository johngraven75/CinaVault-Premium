// Live TV player preferences (the gear button on the player). Stored per
// device in localStorage; pure helpers turn them into hls.js / mpegts.js config.

export type LatencyMode = "low" | "balanced" | "stable";
export type QualityPreference = "auto" | "highest" | "lowest";
export type VideoFit = "contain" | "cover" | "fill";

export interface PlayerSettings {
  /** How close to live the player stays versus how much it buffers. */
  latency: LatencyMode;
  /** Forward buffer target in seconds. */
  bufferSeconds: number;
  /** HLS rendition choice; ignored for single-bitrate MPEG-TS. */
  quality: QualityPreference;
  /** Reconnects after a network drop before showing an error. */
  reconnectAttempts: number;
  videoFit: VideoFit;
  autoplay: boolean;
}

export const DEFAULT_PLAYER_SETTINGS: Readonly<PlayerSettings> = Object.freeze({
  latency: "low",
  bufferSeconds: 30,
  quality: "auto",
  reconnectAttempts: 3,
  videoFit: "contain",
  autoplay: true,
});

export const PLAYER_SETTINGS_KEY = "cinavault.livetv.player";

export const LATENCY_OPTIONS: ReadonlyArray<{ value: LatencyMode; label: string; hint: string }> = [
  { value: "low", label: "Low latency", hint: "Closest to live. Best for sport on a fast connection." },
  { value: "balanced", label: "Balanced", hint: "A few seconds behind live, fewer stalls." },
  { value: "stable", label: "Stable", hint: "Largest cushion. Use for providers that buffer often." },
];

export const QUALITY_OPTIONS: ReadonlyArray<{ value: QualityPreference; label: string }> = [
  { value: "auto", label: "Auto (adapts to your connection)" },
  { value: "highest", label: "Always highest" },
  { value: "lowest", label: "Data saver (lowest)" },
];

export const FIT_OPTIONS: ReadonlyArray<{ value: VideoFit; label: string }> = [
  { value: "contain", label: "Fit" },
  { value: "cover", label: "Fill (crop)" },
  { value: "fill", label: "Stretch" },
];

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

function pick<T extends string>(value: unknown, allowed: ReadonlyArray<{ value: T }>, fallback: T): T {
  return allowed.some((option) => option.value === value) ? (value as T) : fallback;
}

/** Fills gaps and clamps anything out of range (old or hand-edited storage). */
export function normalizePlayerSettings(raw: unknown): PlayerSettings {
  const input = raw && typeof raw === "object" ? (raw as Record<string, unknown>) : {};
  const number = (value: unknown, fallback: number) =>
    typeof value === "number" && Number.isFinite(value) ? value : fallback;
  return {
    latency: pick(input.latency, LATENCY_OPTIONS, DEFAULT_PLAYER_SETTINGS.latency),
    bufferSeconds: Math.round(clamp(number(input.bufferSeconds, DEFAULT_PLAYER_SETTINGS.bufferSeconds), 10, 120)),
    quality: pick(input.quality, QUALITY_OPTIONS, DEFAULT_PLAYER_SETTINGS.quality),
    reconnectAttempts: Math.round(clamp(number(input.reconnectAttempts, DEFAULT_PLAYER_SETTINGS.reconnectAttempts), 0, 10)),
    videoFit: pick(input.videoFit, FIT_OPTIONS, DEFAULT_PLAYER_SETTINGS.videoFit),
    autoplay: typeof input.autoplay === "boolean" ? input.autoplay : DEFAULT_PLAYER_SETTINGS.autoplay,
  };
}

type StorageLike = Pick<Storage, "getItem" | "setItem">;

function defaultStorage(): StorageLike | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}

export function readPlayerSettings(storage: StorageLike | null = defaultStorage()): PlayerSettings {
  try {
    const raw = storage?.getItem(PLAYER_SETTINGS_KEY);
    return normalizePlayerSettings(raw ? JSON.parse(raw) : null);
  } catch {
    return { ...DEFAULT_PLAYER_SETTINGS };
  }
}

export function savePlayerSettings(settings: PlayerSettings, storage: StorageLike | null = defaultStorage()): void {
  try {
    storage?.setItem(PLAYER_SETTINGS_KEY, JSON.stringify(normalizePlayerSettings(settings)));
  } catch {
    // Storage can be unavailable; the settings still apply this session.
  }
}

/** hls.js options for the chosen settings (workers stay off for the app's CSP). */
export function hlsConfigFor(settings: PlayerSettings) {
  const syncCount = { low: 3, balanced: 4, stable: 6 }[settings.latency];
  return {
    enableWorker: false,
    lowLatencyMode: settings.latency === "low",
    backBufferLength: 30,
    liveDurationInfinity: true,
    liveSyncDurationCount: syncCount,
    maxBufferLength: settings.bufferSeconds,
    maxMaxBufferLength: settings.bufferSeconds * 2,
  };
}

/** mpegts.js options for the chosen settings. */
export function mpegtsConfigFor(settings: PlayerSettings) {
  const chasing = settings.latency !== "stable";
  return {
    enableWorker: false,
    lazyLoad: false,
    enableStashBuffer: settings.latency !== "low",
    liveBufferLatencyChasing: chasing,
    liveBufferLatencyMaxLatency: settings.latency === "low" ? 6 : 12,
    liveBufferLatencyMinRemain: settings.latency === "low" ? 1.5 : 3,
  };
}

/** The hls.js level to lock, or -1 to let it adapt. */
export function pickHlsLevel(quality: QualityPreference, levelCount: number): number {
  if (levelCount <= 1 || quality === "auto") return -1;
  return quality === "highest" ? levelCount - 1 : 0;
}
