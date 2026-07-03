export type MediaRowItem = {
  path?: string;
  filePath?: string;
  name?: string;
  title?: string;
  type?: string;
  mediaType?: string;
  mimeType?: string;
  isPoster?: boolean;
  isBackdrop?: boolean;
  isThumbnail?: boolean;
};

const IMAGE_EXTENSIONS = /\.(jpg|jpeg|png|webp|gif|bmp|tiff|avif)$/i;
const VIDEO_EXTENSIONS = /\.(mp4|mkv|avi|mov|wmv|m4v|webm|ts|m2ts|mpg|mpeg|flv)$/i;
const SIDECAR_IMAGE_NAMES =
  /(^|[\\\/\s._-])(poster|cover|folder|fanart|backdrop|banner|thumb|thumbnail|logo|clearlogo|clearart|disc|landscape|screenshot|chapter|scene)([\\\/\s._-]|$)/i;

export function getMediaRowItemPath(item: MediaRowItem): string {
  return item.path || item.filePath || item.name || item.title || "";
}

export function isActualPlayableMedia(item: MediaRowItem): boolean {
  const path = getMediaRowItemPath(item);
  const mime = item.mimeType || "";
  const mediaType = item.mediaType || item.type || "";

  if (VIDEO_EXTENSIONS.test(path)) return true;
  if (/video/i.test(mime)) return true;
  if (/movie|episode|video/i.test(mediaType) && !IMAGE_EXTENSIONS.test(path)) return true;

  return false;
}

export function isSidecarArtworkImage(item: MediaRowItem): boolean {
  const path = getMediaRowItemPath(item);
  const mime = item.mimeType || "";
  const mediaType = item.mediaType || item.type || "";

  if (item.isPoster || item.isBackdrop || item.isThumbnail) return true;
  if (/image|photo|picture|poster|artwork|backdrop|thumbnail/i.test(mime)) return true;
  if (/image|photo|picture|poster|artwork|backdrop|thumbnail/i.test(mediaType)) return true;
  if (IMAGE_EXTENSIONS.test(path)) return true;
  if (SIDECAR_IMAGE_NAMES.test(path)) return true;

  return false;
}

export function cleanMediaRowItems<T extends MediaRowItem>(items: T[]): T[] {
  return items.filter((item) => isActualPlayableMedia(item) && !isSidecarArtworkImage(item));
}
