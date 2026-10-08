// Typed access to the automatic collections (switch "collection_auto"),
// built by src-tauri/src/collections.rs from series, franchises and genres.
import { invoke } from "@tauri-apps/api/core";

import type { MediaItem } from "../store/appStore";
import { groupCollections, type CollectionKind, type CollectionSummary } from "../features/library/libraryAutomation.ts";

export { groupCollections };
export type { CollectionKind, CollectionSummary };

export interface CollectionRebuildReport {
  series: number;
  franchises: number;
  genres: number;
  itemsGrouped: number;
  franchiseLookups: number;
  errors: string[];
}

/** All collections; empty while Auto Collections is off. */
export function listCollections(): Promise<CollectionSummary[]> {
  return invoke<CollectionSummary[]>("collections_list");
}

/** The titles in one collection, in collection order (episode order, release year, or rating). */
export function collectionItems(collectionId: number): Promise<MediaItem[]> {
  return invoke<MediaItem[]>("collection_items", { collectionId });
}

/** Looks up TMDB franchises for new movies (when a TMDB key is stored) and regroups the library. */
export function rebuildCollections(): Promise<CollectionRebuildReport> {
  return invoke<CollectionRebuildReport>("collections_rebuild");
}
