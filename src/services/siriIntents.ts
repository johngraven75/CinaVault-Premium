// CinaVault Premium — Siri intent protocol (the "link up" between Apple's
// App Intents / Siri and the running app). This module is pure logic with no
// DOM, Tauri or React imports, so it is unit tested and shared by the in-app
// router (siriBridge.ts) and any future transport.
//
// Siri requests arrive as a custom-scheme URL. On iOS the App Intents layer
// (platforms/ios/siri) opens the app and hands the webview one of these URLs;
// the router parses it here, matches it against the loaded library, and turns
// it into a concrete plan the app can execute with functions it already has
// (playback.ts, the app store, the holo agent).

import type { MediaItem } from "../store/appStore";

/** Custom URL scheme the iOS App Intents open the app with. */
export const SIRI_URL_SCHEME = "cinavault";

/** Every spoken action Siri can drive. */
export type SiriIntentKind = "play" | "search" | "pause" | "resume" | "skip" | "ask";

const INTENT_KINDS: readonly SiriIntentKind[] = ["play", "search", "pause", "resume", "skip", "ask"];

export interface SiriIntent {
  kind: SiriIntentKind;
  /** play → title; search → query; ask → question. Absent for transport controls. */
  query?: string;
}

/**
 * Parses a `cinavault://<kind>?...` URL into a typed intent, or null when it is
 * not a Siri URL or names an unknown action. Accepts the query under any of the
 * common parameter names so the Swift layer and hand-written Shortcuts agree.
 */
export function parseSiriIntent(raw: string): SiriIntent | null {
  if (typeof raw !== "string" || raw.trim() === "") return null;
  let url: URL;
  try {
    url = new URL(raw.trim());
  } catch {
    return null;
  }
  // URL keeps the trailing colon on the protocol, e.g. "cinavault:".
  if (url.protocol.replace(/:$/, "").toLowerCase() !== SIRI_URL_SCHEME) return null;

  // The action is the host for `scheme://play` and the first path segment for
  // `scheme:///play`; tolerate both so Shortcuts authored by hand still work.
  const host = url.hostname.toLowerCase();
  const firstPath = url.pathname.replace(/^\/+/, "").split("/")[0]?.toLowerCase() ?? "";
  const kindRaw = host || firstPath;
  if (!isIntentKind(kindRaw)) return null;

  const params = url.searchParams;
  const query =
    params.get("title") ??
    params.get("q") ??
    params.get("query") ??
    params.get("text") ??
    undefined;
  const trimmed = query?.trim();

  const intent: SiriIntent = { kind: kindRaw };
  if (trimmed) intent.query = trimmed;

  // play/search/ask are meaningless without a subject.
  if ((kindRaw === "play" || kindRaw === "search" || kindRaw === "ask") && !intent.query) {
    return null;
  }
  return intent;
}

function isIntentKind(value: string): value is SiriIntentKind {
  return (INTENT_KINDS as readonly string[]).includes(value);
}

/** Lower-cases, strips punctuation and collapses whitespace for forgiving matching. */
export function normalizeTitle(value: string): string {
  return value
    .toLowerCase()
    .normalize("NFKD")
    .replace(/[^\p{Letter}\p{Number}\s]/gu, " ")
    .replace(/\s+/g, " ")
    .trim();
}

/**
 * Finds the library item a spoken title most likely refers to. Tries, in order:
 * exact normalized equality, prefix, substring, then token-overlap score. Siri's
 * transcription is rarely exact, so this stays deliberately forgiving but returns
 * null when nothing shares a single word.
 */
export function matchLibraryItem(items: readonly MediaItem[], spokenTitle: string): MediaItem | null {
  const wanted = normalizeTitle(spokenTitle);
  if (!wanted || items.length === 0) return null;
  const wantedTokens = new Set(wanted.split(" ").filter(Boolean));

  let best: MediaItem | null = null;
  let bestScore = 0;

  for (const item of items) {
    const title = normalizeTitle(item.title ?? "");
    if (!title) continue;

    let score = 0;
    if (title === wanted) {
      score = 1000;
    } else if (title.startsWith(wanted) || wanted.startsWith(title)) {
      score = 800 - Math.abs(title.length - wanted.length);
    } else if (title.includes(wanted) || wanted.includes(title)) {
      score = 600 - Math.abs(title.length - wanted.length);
    } else {
      const titleTokens = title.split(" ").filter(Boolean);
      const shared = titleTokens.filter((t) => wantedTokens.has(t)).length;
      if (shared === 0) continue;
      // Favour high overlap relative to both titles' lengths.
      score = (shared / Math.max(titleTokens.length, wantedTokens.size)) * 400;
    }

    if (score > bestScore) {
      bestScore = score;
      best = item;
    }
  }
  return best;
}

/** A resolved, app-executable description of what a Siri intent should do. */
export type SiriPlan =
  | { action: "play"; item: MediaItem }
  | { action: "search"; query: string; reason?: "no-match" }
  | { action: "pause" }
  | { action: "resume" }
  | { action: "skip" }
  | { action: "ask"; question: string };

/**
 * Turns a parsed intent plus the currently loaded library into a concrete plan.
 * A "play X" with no confident match degrades to a search for X (bias to action:
 * Siri still takes the user somewhere useful) with reason "no-match".
 */
export function planSiriIntent(intent: SiriIntent, items: readonly MediaItem[]): SiriPlan {
  switch (intent.kind) {
    case "play": {
      const item = intent.query ? matchLibraryItem(items, intent.query) : null;
      if (item) return { action: "play", item };
      return { action: "search", query: intent.query ?? "", reason: "no-match" };
    }
    case "search":
      return { action: "search", query: intent.query ?? "" };
    case "ask":
      return { action: "ask", question: intent.query ?? "" };
    case "pause":
      return { action: "pause" };
    case "resume":
      return { action: "resume" };
    case "skip":
      return { action: "skip" };
  }
}
