// Pure helpers for the media-server switches' panels (API keys, bandwidth,
// cache) and the activity log / webhook panels. No React, no Tauri.

export interface ApiKeySummary {
  id: number;
  name: string;
  prefix: string;
  permissions: string[];
  createdAt: string;
  lastUsedAt?: string | null;
  revokedAt?: string | null;
}

export interface ActivityEntry {
  id: number;
  at: string;
  profile_id: number;
  kind: string;
  title: string;
  detail: unknown;
}

export const API_KEY_PERMISSIONS = [
  { value: "library:read", label: "Read library" },
  { value: "stream:play", label: "Stream" },
] as const;

export const BANDWIDTH_DEFAULT_MBPS = 20;
export const CACHE_DEFAULTS = { artworkCacheMb: 64, libraryMaxAge: 30 };

/** Event families a webhook can filter on (prefix match on the event kind). */
export const WEBHOOK_EVENT_GROUPS = [
  "playback",
  "scan",
  "profile",
  "watchlist",
  "server",
  "remote",
  "apikey",
  "parental",
] as const;

/** Cap in Mbps from user text; null when it is not a usable number. */
export function parseMbps(text: string): number | null {
  const value = Number(String(text).trim());
  if (!Number.isFinite(value) || value < 0.5 || value > 10000) return null;
  return Math.round(value * 10) / 10;
}

/** Cap from the switch config, falling back to the older Remote Access setting, then 20. */
export function effectiveMbps(config: { mbps?: unknown } | undefined, legacySetting?: string): number {
  return (
    parseMbps(String(config?.mbps ?? "")) ??
    parseMbps(legacySetting ?? "") ??
    BANDWIDTH_DEFAULT_MBPS
  );
}

/** Rough real-world meaning of a cap, for the panel hint. */
export function mbpsHint(mbps: number): string {
  if (mbps >= 25) return "enough for 4K";
  if (mbps >= 8) return "enough for 1080p";
  if (mbps >= 4) return "enough for 720p";
  return "standard definition";
}

export function apiKeyStatus(key: ApiKeySummary): "active" | "revoked" {
  return key.revokedAt ? "revoked" : "active";
}

export function permissionLabels(permissions: string[]): string {
  return permissions
    .map((value) => API_KEY_PERMISSIONS.find((p) => p.value === value)?.label ?? value)
    .join(", ");
}

/** One URL per line; returns the valid http(s) URLs and the rejected lines. */
export function parseWebhookUrls(text: string): { urls: string[]; invalid: string[] } {
  const urls: string[] = [];
  const invalid: string[] = [];
  for (const raw of text.split(/\s*[\n,]\s*/)) {
    const line = raw.trim();
    if (!line) continue;
    let ok = false;
    try {
      const url = new URL(line);
      ok = url.protocol === "https:" || url.protocol === "http:";
    } catch {
      ok = false;
    }
    if (!ok) invalid.push(line);
    else if (!urls.includes(line)) urls.push(line);
  }
  return { urls, invalid };
}

/** Same matching the back end applies (user_data::webhook_targets). */
export function webhookWants(events: string[], kind: string): boolean {
  return events.length === 0 || events.some((event) => kind === event || kind.startsWith(`${event}.`));
}

export function activityGroup(kind: string): string {
  return kind.split(".")[0] || kind;
}

/** "3 min ago" style label relative to `now`. */
export function relativeTime(at: string, now: Date = new Date()): string {
  const then = new Date(at).getTime();
  if (!Number.isFinite(then)) return at;
  const seconds = Math.max(0, Math.round((now.getTime() - then) / 1000));
  if (seconds < 45) return "just now";
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours} h ago`;
  const days = Math.round(hours / 24);
  return `${days} d ago`;
}
