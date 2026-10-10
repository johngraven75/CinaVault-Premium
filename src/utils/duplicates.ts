// Pure helpers for the duplicate finder (src/components/library/DuplicateFinder.tsx).
export type DuplicateFile = { id: number; path: string; name: string; size: number; hash?: string | null };
export type DuplicateGroup = { key: string; count: number; total_size: number; files: DuplicateFile[] };
export type DuplicateScanResult = { groups: DuplicateGroup[]; total_wasted_bytes: number; scanned_files: number };

export const DUPLICATE_MODES = [
  { id: "name_size", label: "Same name and size" },
  { id: "work", label: "Same title (any quality)" },
  { id: "content", label: "Same content (slower)" },
  { id: "size", label: "Same size" },
  { id: "name", label: "Same name" },
] as const;

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const exponent = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
  const value = bytes / 1024 ** exponent;
  return `${value >= 10 || exponent === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[exponent]}`;
}

/** The copy the back end treats as the keeper: the largest file, first on ties. */
export function keeperId(group: DuplicateGroup): number | undefined {
  let best: DuplicateFile | undefined;
  for (const file of group.files) if (!best || file.size > best.size) best = file;
  return best?.id;
}
