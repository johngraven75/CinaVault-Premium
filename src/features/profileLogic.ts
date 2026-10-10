// Pure rules for profiles and parental controls, shared by the profile
// switcher, the settings panels and Node tests. No React, no Tauri.

export interface Profile {
  id: number;
  name: string;
  color: string;
  restricted: boolean;
  created_at: string;
}

export interface ParentalStatus {
  enabled: boolean;
  hasPin: boolean;
  unlocked: boolean;
  activeRestricted: boolean;
  ratedItems: number;
  unratedItems: number;
  maxRatingChoices: string[];
}

export interface ParentalConfig {
  maxRating: string;
  blockedGenres: string[];
  blockAdult: boolean;
  blockUnrated: boolean;
  region: string;
}

export const OWNER_PROFILE_ID = 1;
export const PROFILE_COLORS = ["#38bdf8", "#f472b6", "#a78bfa", "#34d399", "#fbbf24", "#f87171"];
export const PARENTAL_DEFAULTS: ParentalConfig = {
  maxRating: "PG",
  blockedGenres: [],
  blockAdult: true,
  blockUnrated: true,
  region: "US",
};
/** Window events that make the library and the shelves reload. */
export const PROFILE_CHANGE_EVENTS = ["cinavault:profile-changed", "cinavault:library-refresh"] as const;

/** Error text for a profile name, or null when it is fine (the back end's rule). */
export function profileNameError(name: string): string | null {
  const length = [...name.trim()].length;
  return length >= 1 && length <= 40 ? null : "A profile name needs 1 to 40 characters";
}

export function isHexColor(color: string): boolean {
  return /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.test(color.trim());
}

/** Initials for an avatar badge: "Movie Night" -> "MN". */
export function profileInitials(name: string): string {
  const words = name.trim().split(/\s+/).filter(Boolean);
  if (words.length === 0) return "?";
  const letters = words.length === 1 ? [...words[0]].slice(0, 2) : [words[0][0], words[1][0]];
  return letters.join("").toUpperCase();
}

/**
 * Whether switching from the active profile to `target` asks for the PIN:
 * leaving a restricted profile for an unrestricted one while parental controls
 * are on and a PIN is set. The back end enforces the same rule.
 */
export function switchNeedsPin(active: Profile | null, target: Profile, parental: ParentalStatus | null): boolean {
  if (!parental?.enabled || !parental.hasPin || parental.unlocked) return false;
  return Boolean(active?.restricted) && !target.restricted;
}

export function pinError(pin: string): string | null {
  return /^\d{4,8}$/.test(pin.trim()) ? null : "The PIN must be 4 to 8 digits";
}

/** "Horror, true crime ,,Horror" -> ["Horror", "true crime"] (case-insensitive de-dup). */
export function parseGenres(text: string): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const raw of text.split(/[,;\n]/)) {
    const genre = raw.trim();
    if (!genre || seen.has(genre.toLowerCase())) continue;
    seen.add(genre.toLowerCase());
    out.push(genre);
  }
  return out;
}

export function parentalConfig(saved: Partial<ParentalConfig> | undefined): ParentalConfig {
  return {
    ...PARENTAL_DEFAULTS,
    ...saved,
    blockedGenres: Array.isArray(saved?.blockedGenres) ? saved!.blockedGenres.filter((g) => typeof g === "string") : [],
  };
}

/** Back-end errors that mean "ask for the PIN and retry". */
export function isPinRequired(error: unknown): boolean {
  return /^PIN required/i.test(String(error ?? ""));
}
