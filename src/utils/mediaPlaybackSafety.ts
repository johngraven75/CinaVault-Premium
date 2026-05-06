import type { MediaItem } from "../store/appStore";

const VIDEO_EXTENSIONS = new Set([
  ".mp4", ".mkv", ".avi", ".mov", ".wmv", ".flv", ".webm", ".m4v", ".mpg", ".mpeg",
  ".ts", ".m2ts", ".vob", ".ogv", ".3gp", ".divx", ".rm", ".rmvb", ".asf",
]);

const AUDIO_EXTENSIONS = new Set([
  ".mp3", ".flac", ".aac", ".ogg", ".wma", ".wav", ".m4a", ".opus", ".alac", ".aiff",
]);

function normalizePath(filePath: string | undefined): string {
  return (filePath ?? "").trim().replaceAll("/", "\\").toLowerCase();
}

function hasSupportedPlayableExtension(filePath: string): boolean {
  for (const ext of [...VIDEO_EXTENSIONS, ...AUDIO_EXTENSIONS]) {
    if (filePath.endsWith(ext)) {
      return true;
    }
  }
  return false;
}

function isGeneratedChapterImagePath(filePath: string): boolean {
  return filePath.includes("\\_chapters\\") || filePath.includes("_chapters\\chapter_");
}

export function isLibraryDisplayableMediaItem(item: Pick<MediaItem, "file_path" | "media_type">): boolean {
  const filePath = normalizePath(item.file_path);
  if (!filePath) return false;
  if (isGeneratedChapterImagePath(filePath)) return false;
  return true;
}

export function canPlayMediaItem(item: Pick<MediaItem, "file_path" | "media_type">): boolean {
  const filePath = normalizePath(item.file_path);
  if (!isLibraryDisplayableMediaItem(item)) return false;
  return hasSupportedPlayableExtension(filePath);
}
