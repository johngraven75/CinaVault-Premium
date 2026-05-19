import type { MediaItem } from "../store/appStore";

type ArtworkFields = Pick<MediaItem, "poster_path" | "backdrop_path">;

function cleanArtworkPath(path?: string | null): string | undefined {
  const trimmed = path?.trim();
  return trimmed ? trimmed : undefined;
}

export function pickPosterImagePath(item?: ArtworkFields | null): string | undefined {
  if (!item) return undefined;
  return cleanArtworkPath(item.poster_path) ?? cleanArtworkPath(item.backdrop_path);
}

export function pickBackdropImagePath(item?: ArtworkFields | null): string | undefined {
  if (!item) return undefined;
  return cleanArtworkPath(item.backdrop_path) ?? cleanArtworkPath(item.poster_path);
}
