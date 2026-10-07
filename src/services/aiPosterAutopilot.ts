// Background AI poster check that runs at boot with no user input.
//
// After the local CLIP model warms up, unverified library items that already
// have a poster are checked: a poster that matches its title marks the item
// verified (`verify_media_item`); a poster that doesn't is reported as flagged
// so the library agent can re-fetch artwork. Nothing is deleted or rewritten.

import { invoke } from "@tauri-apps/api/core";
import type { MediaItem } from "../store/appStore";
import type { PosterVerification } from "./posterIdentification.ts";

export const AI_POSTER_AUTOPILOT_SETTING = "ai_poster_autoverify";
export const DEFAULT_PASS_LIMIT = 60;

export interface PosterPassResult {
  checked: number;
  verified: number[];
  flagged: number[];
  skipped: number;
  errors: number;
}

export interface PosterPassDeps {
  listUnverified: () => Promise<MediaItem[]>;
  posterSource: (item: MediaItem) => Promise<string | null>;
  verify: (item: MediaItem, src: string) => Promise<PosterVerification>;
  markVerified: (id: number) => Promise<void>;
  shouldStop?: () => boolean;
}

/** Items worth checking: have an id, a title and a poster; adult items last; capped. */
export function selectPassCandidates(items: readonly MediaItem[], limit = DEFAULT_PASS_LIMIT): MediaItem[] {
  return items
    .filter(
      (item) =>
        typeof item.id === "number" &&
        !item.verified &&
        item.title.trim().length > 0 &&
        typeof item.poster_path === "string" &&
        item.poster_path.trim().length > 0,
    )
    .sort((a, b) => Number(a.media_type === "adult") - Number(b.media_type === "adult"))
    .slice(0, Math.max(0, limit));
}

/** Setting value → enabled. Anything but an explicit "false" keeps the autopilot on. */
export function isAutopilotEnabled(settingValue: string | null | undefined): boolean {
  return (settingValue ?? "").trim().toLowerCase() !== "false";
}

/** Runs one pass; each item is isolated so one bad poster never stops the pass. */
export async function runPosterPass(deps: PosterPassDeps, limit = DEFAULT_PASS_LIMIT): Promise<PosterPassResult> {
  const result: PosterPassResult = { checked: 0, verified: [], flagged: [], skipped: 0, errors: 0 };
  const candidates = selectPassCandidates(await deps.listUnverified(), limit);
  for (const item of candidates) {
    if (deps.shouldStop?.()) break;
    const id = item.id as number;
    try {
      const src = await deps.posterSource(item);
      if (!src) {
        result.skipped += 1;
        continue;
      }
      const verdict = await deps.verify(item, src);
      result.checked += 1;
      if (verdict.ok) {
        await deps.markVerified(id);
        result.verified.push(id);
      } else if (verdict.posterLikelihood >= 0.5) {
        // It is poster art, just not for this title.
        result.flagged.push(id);
      }
    } catch {
      result.errors += 1;
    }
  }
  return result;
}

/** Local poster files go through the existing authorized `get_poster_data_url`. */
async function posterSourceFor(item: MediaItem): Promise<string | null> {
  const poster = item.poster_path?.trim();
  if (!poster) return null;
  if (/^https?:\/\//i.test(poster) || poster.startsWith("data:")) return poster;
  return invoke<string>("get_poster_data_url", { path: poster });
}

let running = false;

/** Boot entry point: warms the model, then runs a pass. Never throws. */
export async function startPosterAutopilot(): Promise<PosterPassResult | null> {
  if (running) return null;
  running = true;
  try {
    const setting = await invoke<string | null>("get_setting", { key: AI_POSTER_AUTOPILOT_SETTING }).catch(
      () => null,
    );
    if (!isAutopilotEnabled(setting)) return null;
    const [{ warmUpVision }, { verifyPoster }] = await Promise.all([
      import("./localVision.ts"),
      import("./posterIdentification.ts"),
    ]);
    if (!(await warmUpVision())) return null;
    return await runPosterPass({
      listUnverified: () => invoke<MediaItem[]>("get_unverified_media"),
      posterSource: posterSourceFor,
      verify: (item, src) => verifyPoster(item, src),
      markVerified: (id) => invoke("verify_media_item", { id }),
    });
  } catch {
    return null;
  } finally {
    running = false;
  }
}
