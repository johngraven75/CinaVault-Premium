import type { MediaItem, MediaSource, MetadataProvider } from "../store/appStore";

/** Build-time edition guard. Vite replaces VITE_STORE_SAFE for the MS-v1 build. */
export const IS_STORE_SAFE = import.meta.env.VITE_STORE_SAFE === "true";

export function isStoreSafeMedia(item: Pick<MediaItem, "media_type">): boolean {
  return !IS_STORE_SAFE || item.media_type?.trim().toLowerCase() !== "adult";
}

export function filterStoreSafeMedia(items: MediaItem[]): MediaItem[] {
  return IS_STORE_SAFE ? items.filter(isStoreSafeMedia) : items;
}

export function filterStoreSafeSources(items: MediaSource[]): MediaSource[] {
  return IS_STORE_SAFE
    ? items.filter((item) => item.source_type.trim().toLowerCase() !== "adult")
    : items;
}

export function filterStoreSafeProviders(items: MetadataProvider[]): MetadataProvider[] {
  return IS_STORE_SAFE
    ? items.filter((item) => item.category.trim().toLowerCase() !== "adult")
    : items;
}
