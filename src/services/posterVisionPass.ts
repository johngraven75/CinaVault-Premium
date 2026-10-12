// Runs the poster vision skill over the library after a scan (and once at
// boot): every title with a poster and no vision record gets one. Empty genre
// and year fields are filled; TMDB/OMDb/NFO values are never replaced (the
// back end only writes empty fields, see src-tauri/src/media_vision.rs).

import { invoke } from "@tauri-apps/api/core";
import type { MediaItem } from "../store/appStore";
import {
  LOCAL_MODEL_LABEL,
  POSTER_READ_MODEL,
  genreField,
  yearToFill,
  type PosterInsights,
  type PosterRead,
} from "./posterVision.ts";

export const VISION_TAGS_SETTING = "ai_vision_tags";
export const VISION_CLOUD_READ_SETTING = "ai_vision_cloud_read";
export const DEFAULT_VISION_LIMIT = 40;
/** Cloud reads per pass, so one big scan doesn't spend the free monthly credits. */
export const DEFAULT_CLOUD_READS = 15;

export interface VisionRecordPayload {
  kind: string | null;
  genres: string[];
  moods: string[];
  era: string | null;
  tags: string[];
  caption: string | null;
  readTitle: string | null;
  readYear: number | null;
  models: string[];
  fillGenre: string | null;
  fillYear: number | null;
}

export interface VisionPassResult {
  analyzed: number;
  genresFilled: number;
  yearsFilled: number;
  cloudReads: number;
  skipped: number;
  errors: number;
}

export interface VisionPassDeps {
  listPending: (limit: number) => Promise<MediaItem[]>;
  posterSource: (item: MediaItem) => Promise<string | null>;
  analyze: (src: string) => Promise<PosterInsights>;
  /** Optional cloud read; omitted when no Hugging Face token is set or it's turned off. */
  read?: (src: string) => Promise<PosterRead | null>;
  save: (id: number, record: VisionRecordPayload) => Promise<{ genreFilled: boolean; yearFilled: boolean }>;
  cloudReadLimit?: number;
  shouldStop?: () => boolean;
}

/** Setting value → enabled; anything but an explicit "false" keeps it on. */
export function isSettingOn(value: string | null | undefined): boolean {
  return (value ?? "").trim().toLowerCase() !== "false";
}

export function buildRecord(item: MediaItem, insights: PosterInsights, read: PosterRead | null): VisionRecordPayload {
  const hasGenre = typeof item.genre === "string" && item.genre.trim().length > 0;
  return {
    kind: insights.kind,
    genres: insights.genres,
    moods: insights.moods,
    era: insights.era,
    tags: insights.tags,
    caption: read?.caption ?? null,
    readTitle: read?.title ?? null,
    readYear: read?.year ?? null,
    models: read ? [LOCAL_MODEL_LABEL, POSTER_READ_MODEL] : [LOCAL_MODEL_LABEL],
    fillGenre: hasGenre ? null : genreField(insights),
    fillYear: yearToFill(item.title, item.year ?? null, read),
  };
}

/** One pass; each title is isolated so one bad poster never stops the rest. */
export async function runVisionPass(deps: VisionPassDeps, limit = DEFAULT_VISION_LIMIT): Promise<VisionPassResult> {
  const result: VisionPassResult = { analyzed: 0, genresFilled: 0, yearsFilled: 0, cloudReads: 0, skipped: 0, errors: 0 };
  const cloudLimit = deps.cloudReadLimit ?? DEFAULT_CLOUD_READS;
  const items = await deps.listPending(limit);
  for (const item of items) {
    if (deps.shouldStop?.()) break;
    if (typeof item.id !== "number") {
      result.skipped += 1;
      continue;
    }
    try {
      const src = await deps.posterSource(item);
      if (!src) {
        result.skipped += 1;
        continue;
      }
      const insights = await deps.analyze(src);
      // Adult titles stay local: their artwork is never sent to a cloud model.
      const mayRead = deps.read && item.media_type !== "adult" && result.cloudReads < cloudLimit && insights.isPoster;
      let read: PosterRead | null = null;
      if (mayRead && deps.read) {
        result.cloudReads += 1;
        read = await deps.read(src).catch(() => null);
      }
      const saved = await deps.save(item.id, buildRecord(item, insights, read));
      result.analyzed += 1;
      if (saved.genreFilled) result.genresFilled += 1;
      if (saved.yearFilled) result.yearsFilled += 1;
    } catch {
      result.errors += 1;
    }
  }
  return result;
}

function posterSourceFor(item: MediaItem): Promise<string | null> {
  const poster = item.poster_path?.trim();
  if (!poster) return Promise.resolve(null);
  if (/^https?:\/\//i.test(poster) || poster.startsWith("data:")) return Promise.resolve(poster);
  return invoke<string>("get_poster_data_url", { path: poster });
}

let running: Promise<VisionPassResult | null> | null = null;

/** Entry point after a scan or at boot. Never throws; concurrent calls share one pass. */
export function startVisionPass(limit = DEFAULT_VISION_LIMIT): Promise<VisionPassResult | null> {
  if (running) return running;
  running = (async () => {
    const getSetting = (key: string) => invoke<string | null>("get_setting", { key }).catch(() => null);
    if (!isSettingOn(await getSetting(VISION_TAGS_SETTING))) return null;
    const [{ warmUpVision }, vision] = await Promise.all([import("./localVision.ts"), import("./posterVision.ts")]);
    if (!(await warmUpVision())) return null;
    const cloud = isSettingOn(await getSetting(VISION_CLOUD_READ_SETTING)) && (await vision.hasHuggingFaceToken());
    return runVisionPass(
      {
        listPending: (count) => invoke<MediaItem[]>("media_vision_pending", { limit: count }),
        posterSource: posterSourceFor,
        analyze: (src) => vision.analyzePoster(src),
        read: cloud ? (src) => vision.readPosterText(src) : undefined,
        save: (id, record) => invoke("media_vision_save", { id, record }),
      },
      limit,
    );
  })()
    .catch(() => null)
    .finally(() => {
      running = null;
    });
  return running;
}

export function summarizeVisionPass(result: VisionPassResult): string | null {
  if (result.analyzed === 0) return null;
  const filled = [
    result.genresFilled ? `${result.genresFilled} genre${result.genresFilled === 1 ? "" : "s"}` : "",
    result.yearsFilled ? `${result.yearsFilled} year${result.yearsFilled === 1 ? "" : "s"}` : "",
  ].filter(Boolean);
  return `AI vision read ${result.analyzed} poster${result.analyzed === 1 ? "" : "s"}${filled.length ? ` and filled in ${filled.join(" and ")}` : ""}.`;
}
