// CinaVault Premium — Siri intent router. Bridges the iOS App Intents layer
// (platforms/ios/siri) to the running web app. On iOS the native App Intent
// opens the app and the generated app delegate hands the webview a
// `cinavault://…` URL by calling `window.__cinavaultSiriIntent(url)` (see
// platforms/ios/siri/README.md). This module installs that global, parses the
// URL with the pure logic in siriIntents.ts, and executes the resulting plan
// against functions the app already has.
//
// All app-specific effects arrive as injected callbacks (SiriBridgeDeps) so the
// routing is unit tested with plain mocks and carries no hard dependency on the
// store, the player or the agent.

import {
  type SiriPlan,
  parseSiriIntent,
  planSiriIntent,
} from "./siriIntents.ts";
import type { MediaItem } from "../store/appStore";

/** DOM event the holo agent listens for to answer a spoken question aloud. */
export const SIRI_ASK_EVENT = "cinavault:siri-ask";
/** Global the native layer calls with a `cinavault://` URL. */
export const SIRI_GLOBAL = "__cinavaultSiriIntent";

export interface SiriBridgeDeps {
  /** Current library, used to resolve a spoken title to an item. */
  getLibrary: () => readonly MediaItem[];
  /** Switch to the Library/home view (for play and search). */
  showLibrary: () => void;
  /** Set the library search box (search intent, and the no-match fallback of play). */
  setSearch: (query: string) => void;
  /** Start playback of a resolved item. */
  playItem: (item: MediaItem) => void | Promise<void>;
  /** Pause / resume / skip the open player; returns whether a player handled it. */
  control: (command: "pause" | "resume" | "skip") => boolean | Promise<boolean>;
  /** Route a spoken question into the holo agent (which answers aloud). */
  ask: (question: string) => void;
}

/** The outcome of a routed intent, returned for telemetry and tests. */
export interface SiriResult {
  ok: boolean;
  plan?: SiriPlan;
  /** Set when the URL was not a recognisable Siri intent. */
  error?: "unrecognised";
}

/** Parses and executes one `cinavault://` URL. Pure routing; effects via deps. */
export async function executeSiriUrl(raw: string, deps: SiriBridgeDeps): Promise<SiriResult> {
  const intent = parseSiriIntent(raw);
  if (!intent) return { ok: false, error: "unrecognised" };
  const plan = planSiriIntent(intent, deps.getLibrary());
  await executeSiriPlan(plan, deps);
  return { ok: true, plan };
}

/** Executes an already-resolved plan. Separated so tests can drive plans directly. */
export async function executeSiriPlan(plan: SiriPlan, deps: SiriBridgeDeps): Promise<void> {
  switch (plan.action) {
    case "play":
      deps.showLibrary();
      await deps.playItem(plan.item);
      return;
    case "search":
      deps.showLibrary();
      deps.setSearch(plan.query);
      return;
    case "ask":
      deps.ask(plan.question);
      return;
    case "pause":
    case "resume":
    case "skip":
      await deps.control(plan.action);
      return;
  }
}

/**
 * Installs `window.__cinavaultSiriIntent`. Any URL the native layer delivered
 * before the app finished mounting is queued on the global as an array and
 * drained here. Returns an uninstall function. Safe to call where there is no
 * window (SSR / tests) — it then does nothing and returns a no-op.
 */
export function installSiriBridge(deps: SiriBridgeDeps): () => void {
  if (typeof window === "undefined") return () => {};
  const w = window as unknown as Record<string, unknown>;

  // Drain anything the native layer buffered before we were ready.
  const pending = w[SIRI_GLOBAL];
  const handler = (url: string): void => {
    void executeSiriUrl(url, deps);
  };
  if (Array.isArray(pending)) {
    for (const url of pending) {
      if (typeof url === "string") handler(url);
    }
  }
  w[SIRI_GLOBAL] = handler;

  return () => {
    if (w[SIRI_GLOBAL] === handler) delete w[SIRI_GLOBAL];
  };
}

/** Fires the holo-agent ask event. Used by the default `ask` dep in App. */
export function emitSiriAsk(question: string): void {
  if (typeof window === "undefined") return;
  const clean = question.trim();
  if (!clean) return;
  window.dispatchEvent(new CustomEvent<string>(SIRI_ASK_EVENT, { detail: clean }));
}
