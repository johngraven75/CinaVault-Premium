// Home screen discovery: typed calls to the discovery and user_data commands
// (src-tauri/src/discovery.rs, src-tauri/src/user_data.rs). Pure shaping
// helpers live in discoveryLogic.ts so tests can import them without Tauri.
import { invoke } from "@tauri-apps/api/core";

import type { MediaItem } from "../store/appStore";
import type { UnifiedEntry } from "./unifiedLibrary";
import {
  continueWatchingItems,
  toShelfItems,
  type DiscoveryEntryDto,
  type ProgressItemDto,
  type ShelfItem,
} from "./discoveryLogic.ts";

export * from "./discoveryLogic.ts";

export interface TrendingShelfResult {
  /** "tmdb", "local" or "off". */
  source: string;
  title: string;
  note: string | null;
  fetched_at: string | null;
  items: ShelfItem[];
}

export interface NewReleasesResult {
  since: string | null;
  checked_at: string;
  count: number;
  items: ShelfItem[];
}

export interface GenreCount {
  name: string;
  count: number;
}

export interface ActiveProfile {
  id: number;
  name: string;
  restricted: boolean;
}

export const activeProfile = () => invoke<ActiveProfile>("profile_active");

export async function fetchContinueWatching(limit = 24): Promise<ShelfItem[]> {
  return continueWatchingItems(await invoke<ProgressItemDto[]>("continue_watching_list", { limit }));
}

export const fetchWatchlist = () => invoke<MediaItem[]>("watchlist_list");

/** Returns whether the title is now on the active profile's watchlist. */
export const toggleWatchlistEntry = (mediaId: number) => invoke<boolean>("watchlist_toggle", { mediaId });

export async function fetchRecommendations(limit = 24): Promise<ShelfItem[]> {
  return toShelfItems(await invoke<DiscoveryEntryDto[]>("discovery_recommendations", { limit }));
}

export async function fetchSimilar(mediaId: number, limit = 12): Promise<ShelfItem[]> {
  return toShelfItems(await invoke<DiscoveryEntryDto[]>("discovery_similar", { mediaId, limit }));
}

export async function fetchTrending(limit = 20, localOnly = false): Promise<TrendingShelfResult> {
  const result = await invoke<Omit<TrendingShelfResult, "items"> & { items: DiscoveryEntryDto[] }>(
    "discovery_trending",
    { limit, localOnly },
  );
  return { ...result, items: toShelfItems(result.items) };
}

export async function fetchNewReleases(since: string | null, acknowledge: boolean): Promise<NewReleasesResult> {
  const result = await invoke<Omit<NewReleasesResult, "items"> & { items: DiscoveryEntryDto[] }>(
    "discovery_new_releases",
    { since, acknowledge },
  );
  return { ...result, items: toShelfItems(result.items) };
}

export const fetchGenres = () => invoke<GenreCount[]>("discovery_genres");

export const fetchGenreQueue = (genre: string, limit: number) =>
  invoke<UnifiedEntry[]>("discovery_genre_queue", { genre, limit });
