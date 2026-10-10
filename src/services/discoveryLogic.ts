// Pure helpers behind the home screen discovery shelves (no React, no Tauri),
// so Node tests can import them. Ranking itself lives in
// src-tauri/src/discovery.rs; this module shapes its results for the UI.
import type { MediaItem } from "../store/appStore";
import { unifiedEntryToMediaItem, type UnifiedEntry } from "./unifiedLibrary.ts";

/** One card from a discovery command: a unified work plus why it is shown. */
export interface DiscoveryEntryDto extends UnifiedEntry {
  reason: string;
  score: number;
}

export interface ShelfItem {
  item: MediaItem;
  reason?: string;
  /** 0..1 for Continue Watching cards. */
  progress?: number;
  /** Seconds to resume from. */
  resumeAt?: number;
}

export interface ProgressDto {
  media_id: number;
  position: number;
  duration: number;
  finished: boolean;
  updated_at: string;
}

export interface ProgressItemDto {
  item: MediaItem;
  progress: ProgressDto;
}

const TRAILING_PUNCTUATION = new Set([" ", "\t", "\n", "\r", ",", ".", ";", ":", "-"]);

/** Drops trailing whitespace and , . ; : - so a cut preview ends cleanly. */
function trimTrailingPunctuation(text: string): string {
  let end = text.length;
  while (end > 0 && TRAILING_PUNCTUATION.has(text[end - 1])) end -= 1;
  return text.slice(0, end);
}

export function toShelfItems(entries: readonly DiscoveryEntryDto[]): ShelfItem[] {
  return entries.map((entry) => ({ item: unifiedEntryToMediaItem(entry), reason: entry.reason || undefined }));
}

/** Played fraction 0..1; 0 when the duration is unknown. */
export function progressFraction(position: number, duration: number): number {
  if (!Number.isFinite(position) || !Number.isFinite(duration) || duration <= 0) return 0;
  return Math.min(1, Math.max(0, position / duration));
}

function clock(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}` : `${m}:${String(s).padStart(2, "0")}`;
}

export function continueWatchingItems(rows: readonly ProgressItemDto[]): ShelfItem[] {
  return rows.map(({ item, progress }) => {
    const left = Math.max(0, progress.duration - progress.position);
    return {
      item,
      progress: progressFraction(progress.position, progress.duration),
      resumeAt: Math.max(0, progress.position),
      reason: progress.duration > 0 ? `${clock(left)} left` : `Stopped at ${clock(progress.position)}`,
    };
  });
}

/** Fisher-Yates shuffle into a new array; `random` returns [0, 1). */
export function shuffled<T>(items: readonly T[], random: () => number = Math.random): T[] {
  const out = [...items];
  for (let i = out.length - 1; i > 0; i -= 1) {
    const j = Math.min(i, Math.floor(random() * (i + 1)));
    [out[i], out[j]] = [out[j], out[i]];
  }
  return out;
}

export function newReleasesMessage(count: number): string {
  if (count <= 0) return "Nothing new since your last visit";
  return count === 1 ? "1 new title since your last visit" : `${count.toLocaleString("en-US")} new titles since your last visit`;
}

/** Synopsis for the hover preview, cut on a word boundary. */
export function previewText(overview: string | undefined | null, max = 220): string {
  const text = (overview ?? "").replace(/\s+/g, " ").trim();
  if (text.length <= max) return text;
  const cut = text.slice(0, max);
  const space = cut.lastIndexOf(" ");
  return `${trimTrailingPunctuation(space > max * 0.6 ? cut.slice(0, space) : cut)}…`;
}

/** How far one carousel button press scrolls: most of a viewport, whole cards. */
export function carouselStep(viewportWidth: number, cardWidth: number, gap: number): number {
  const pitch = Math.max(1, cardWidth + gap);
  const cards = Math.max(1, Math.floor((viewportWidth * 0.85) / pitch));
  return cards * pitch;
}

/**
 * A vertical wheel turn over a carousel scrolls it sideways, but only while
 * it can still move that way, so the page keeps scrolling at either end.
 */
export function wheelScrollDelta(deltaX: number, deltaY: number, scrollLeft: number, maxScroll: number): number {
  const delta = Math.abs(deltaX) > Math.abs(deltaY) ? deltaX : deltaY;
  if (delta < 0 && scrollLeft <= 0) return 0;
  if (delta > 0 && scrollLeft >= maxScroll - 1) return 0;
  return delta;
}

/** Media ids of every copy of an item (the watchlist stores one per copy). */
export function itemMediaIds(item: MediaItem): number[] {
  const ids = new Set<number>();
  if (typeof item.id === "number" && item.id >= 0) ids.add(item.id);
  for (const copy of item.copies ?? []) if (copy.id >= 0) ids.add(copy.id);
  return [...ids];
}

export function isOnWatchlist(item: MediaItem, watchlistIds: ReadonlySet<number>): boolean {
  return itemMediaIds(item).some((id) => watchlistIds.has(id));
}

/** The genre queue for Genre Radio: playable works, shuffled. */
export function genreRadioQueue(
  entries: readonly UnifiedEntry[],
  canPlay: (item: MediaItem) => boolean,
  random: () => number = Math.random,
): MediaItem[] {
  return shuffled(entries.map(unifiedEntryToMediaItem).filter(canPlay), random);
}

// Switch configs (Advanced > Feature Matrix panels).

export interface TrendingConfig {
  limit: number;
  /** "auto": TMDB when a key is set, else plays on this PC. "local": always plays on this PC. */
  source: "auto" | "local";
}
export const TRENDING_DEFAULTS: TrendingConfig = { limit: 20, source: "auto" };

export interface RecommendationsConfig {
  limit: number;
}
export const RECOMMENDATIONS_DEFAULTS: RecommendationsConfig = { limit: 24 };

export interface GenreRadioConfig {
  /** Most titles queued per station. */
  queueSize: number;
}
export const GENRE_RADIO_DEFAULTS: GenreRadioConfig = { queueSize: 200 };

/** A shelf size from a saved config, kept within 4..60. */
export function shelfLimit(value: unknown, fallback: number): number {
  const n = Math.round(Number(value));
  return Number.isFinite(n) && n > 0 ? Math.min(60, Math.max(4, n)) : fallback;
}

/** A Genre Radio queue size from a saved config, kept within 5..1000. */
export function stationSize(value: unknown): number {
  const n = Math.round(Number(value));
  return Number.isFinite(n) && n > 0 ? Math.min(1000, Math.max(5, n)) : GENRE_RADIO_DEFAULTS.queueSize;
}
