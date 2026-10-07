// Unified library — one card per work across every source.
//
// Calls the Rust `get_unified_library` command (src-tauri/src/library_unify.rs)
// and maps each UnifiedEntry onto the MediaItem shape the existing grids
// already render, adding `work_key`, `copy_count` and `copies`. When the
// command is missing or fails, callers fall back to the paged
// `get_media_items` loader so the library never goes blank.
import { invoke } from "@tauri-apps/api/core";
import type { MediaCopyInfo, MediaItem } from "../store/appStore";

export type { MediaCopyInfo };

export interface UnifiedEntry {
  work_key: string;
  primary: MediaItem;
  copies: MediaCopyInfo[];
  copy_count: number;
  source_ids: number[];
}

export interface UnifiedMediaItem extends MediaItem {
  work_key: string;
  copy_count: number;
  copies: MediaCopyInfo[];
}

function copyFromItem(item: MediaItem): MediaCopyInfo {
  return {
    id: item.id ?? -1,
    file_path: item.file_path,
    source_id: item.source_id ?? null,
    file_size: item.file_size ?? null,
    resolution: item.resolution ?? null,
  };
}

function sanitizeCopy(copy: Partial<MediaCopyInfo> | null | undefined): MediaCopyInfo | null {
  if (!copy || typeof copy.file_path !== "string" || !copy.file_path.trim()) {
    return null;
  }
  return {
    id: typeof copy.id === "number" ? copy.id : -1,
    file_path: copy.file_path,
    source_id: typeof copy.source_id === "number" ? copy.source_id : null,
    file_size: typeof copy.file_size === "number" ? copy.file_size : null,
    resolution:
      typeof copy.resolution === "string" && copy.resolution.trim()
        ? copy.resolution
        : null,
  };
}

/** Maps one UnifiedEntry to a grid card: the primary item plus its copies (primary first). */
export function unifiedEntryToMediaItem(entry: UnifiedEntry): UnifiedMediaItem {
  const primary = entry.primary;
  const copies: MediaCopyInfo[] = [];
  const seen = new Set<string>();
  const push = (copy: MediaCopyInfo | null) => {
    if (!copy) return;
    const key = copy.id >= 0 ? `id:${copy.id}` : `path:${copy.file_path}`;
    if (seen.has(key)) return;
    seen.add(key);
    copies.push(copy);
  };

  const listed = Array.isArray(entry.copies) ? entry.copies : [];
  const primaryCopy =
    listed.find((copy) => primary.id != null && copy?.id === primary.id) ??
    listed.find((copy) => copy?.file_path === primary.file_path);
  push(sanitizeCopy(primaryCopy) ?? copyFromItem(primary));
  for (const copy of listed) push(sanitizeCopy(copy));

  const reported = Number(entry.copy_count);
  const copyCount = Math.max(
    copies.length,
    Number.isFinite(reported) && reported > 0 ? Math.floor(reported) : 1,
  );

  return {
    ...primary,
    work_key:
      entry.work_key ||
      `id:${primary.id ?? primary.file_path}`,
    copy_count: copyCount,
    copies,
  };
}

export function mapUnifiedEntries(entries: UnifiedEntry[]): UnifiedMediaItem[] {
  if (!Array.isArray(entries)) return [];
  return entries
    .filter((entry) => entry && entry.primary && typeof entry.primary.file_path === "string")
    .map(unifiedEntryToMediaItem);
}

/** Wraps plain MediaItems (fallback path) as single-copy cards. */
export function mediaItemsAsSingleCopies(items: MediaItem[]): MediaItem[] {
  return items.map((item) =>
    item.copies && item.copies.length
      ? item
      : { ...item, copy_count: 1, copies: [copyFromItem(item)] },
  );
}

export function hasMultipleCopies(item: Pick<MediaItem, "copy_count" | "copies">): boolean {
  return Math.max(item.copy_count ?? 0, item.copies?.length ?? 0) > 1;
}

export function copyCountLabel(item: Pick<MediaItem, "copy_count" | "copies">): string {
  const count = Math.max(item.copy_count ?? 1, item.copies?.length ?? 1);
  return `×${count} copies`;
}

export function formatFileSize(bytes: number | null | undefined): string {
  if (typeof bytes !== "number" || !Number.isFinite(bytes) || bytes <= 0) {
    return "Unknown size";
  }
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value >= 100 || unit === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[unit]}`;
}

export function copyFileName(copy: Pick<MediaCopyInfo, "file_path">): string {
  return copy.file_path.split(/[\\/]/).filter(Boolean).pop() || copy.file_path;
}

export async function fetchUnifiedLibrary(
  mediaType?: string,
): Promise<UnifiedMediaItem[]> {
  const args =
    mediaType && mediaType !== "all" ? { mediaType } : {};
  const entries = await invoke<UnifiedEntry[]>("get_unified_library", args);
  return mapUnifiedEntries(entries);
}

export interface UnifiedLibraryLoad {
  items: MediaItem[];
  unified: boolean;
  error?: string;
}

/**
 * Loads the unified library; if the command is unavailable or fails, uses the
 * supplied fallback loader (normally a get_media_items page request).
 */
export async function loadUnifiedLibrary(
  mediaType: string | undefined,
  fallback: () => Promise<MediaItem[]>,
): Promise<UnifiedLibraryLoad> {
  try {
    return { items: await fetchUnifiedLibrary(mediaType), unified: true };
  } catch (error) {
    const items = await fallback();
    return {
      items: mediaItemsAsSingleCopies(items),
      unified: false,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}
