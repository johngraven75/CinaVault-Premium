// Pure helpers for the Library & Metadata switches (auto_metadata,
// subtitle_fetch, poster_sync, chapter_thumbs, collection_auto). No React or
// Tauri here so Node tests can import the module directly.

/** Lower-case, de-duplicated subtitle language codes ("EN, pt_BR" -> ["en", "pt-br"]). Mirrors subtitles.rs. */
export function parseLanguageList(text: string): string[] {
  const out: string[] = [];
  for (const raw of text.split(/[\s,;]+/)) {
    const code = raw.trim().toLowerCase().replaceAll("_", "-");
    if (code && code.length <= 7 && /^[a-z-]+$/.test(code) && !out.includes(code)) out.push(code);
  }
  return out;
}

export function formatLanguageList(languages: unknown): string {
  return Array.isArray(languages) ? languages.filter((code) => typeof code === "string").join(", ") : "";
}

/** Chapter thumbnail spacing in minutes, as chapters.rs clamps it (30 s .. 60 min, default 5). */
export function chapterIntervalMinutes(value: unknown): number {
  const minutes = typeof value === "number" ? value : Number(value);
  if (!Number.isFinite(minutes) || minutes <= 0) return 5;
  return Math.min(60, Math.max(0.5, Math.round(minutes * 2) / 2));
}

/** Whole number within [min, max], or the fallback. */
export function boundedInt(value: unknown, fallback: number, min: number, max: number): number {
  const n = typeof value === "number" ? value : Number(value);
  if (!Number.isFinite(n)) return fallback;
  return Math.min(max, Math.max(min, Math.round(n)));
}

function when(at: unknown): string {
  if (typeof at !== "string") return "";
  const date = new Date(at);
  return Number.isNaN(date.getTime()) ? "" : ` (${date.toLocaleString()})`;
}

const count = (value: unknown) => (typeof value === "number" && Number.isFinite(value) ? value : 0);

export interface SubtitleRun {
  status?: string;
  at?: string;
  items?: number;
  downloaded?: number;
  alreadyPresent?: number;
  notFound?: number;
  errors?: string[];
  message?: string;
}

export function describeSubtitleRun(run: SubtitleRun | null | undefined): string {
  if (!run) return "No automatic run yet.";
  if (run.status === "skipped_no_key") return `Skipped: no OpenSubtitles API key${when(run.at)}.`;
  const errors = run.errors?.length ?? 0;
  return (
    `${count(run.downloaded)} downloaded, ${count(run.alreadyPresent)} already present, ` +
    `${count(run.notFound)} not found for ${count(run.items)} title(s)` +
    (errors ? `, ${errors} error(s)` : "") +
    when(run.at)
  );
}

export interface PosterSyncRun {
  trigger?: string;
  at?: string;
  exported?: number;
  unchanged?: number;
  imported?: number;
  errors?: string[];
}

export function describePosterSyncRun(run: PosterSyncRun | null | undefined): string {
  if (!run) return "Not synced yet.";
  const errors = run.errors?.length ?? 0;
  const source = run.trigger === "manual" ? "Sync now" : "After scan";
  return (
    `${source}: ${count(run.exported)} copied to the folder, ${count(run.unchanged)} unchanged, ` +
    `${count(run.imported)} imported` +
    (errors ? `, ${errors} error(s)` : "") +
    when(run.at)
  );
}

export interface PostScanRun {
  at?: string;
  items?: number;
  stopped?: boolean;
  metadata?: { items?: number; updated?: number; errors?: string[] };
  chapterThumbs?: { generated?: number; skipped?: number; errors?: string[] };
  collections?: { series?: number; franchises?: number; genres?: number; error?: string };
}

export function describePostScanRun(run: PostScanRun | null | undefined): string[] {
  if (!run) return ["No scan has added titles yet."];
  const lines = [`${count(run.items)} new title(s)${when(run.at)}${run.stopped ? " (stopped)" : ""}`];
  if (run.metadata) lines.push(`Metadata: ${count(run.metadata.updated)} of ${count(run.metadata.items)} matched`);
  if (run.chapterThumbs)
    lines.push(`Chapter thumbnails: ${count(run.chapterThumbs.generated)} made, ${count(run.chapterThumbs.skipped)} skipped`);
  if (run.collections) {
    lines.push(
      run.collections.error
        ? `Collections: ${run.collections.error}`
        : `Collections: ${count(run.collections.franchises)} franchise, ${count(run.collections.series)} series, ${count(run.collections.genres)} genre`,
    );
  }
  return lines;
}

export type CollectionKind = "franchise" | "series" | "genre";

export interface CollectionSummary {
  id: number;
  key: string;
  name: string;
  kind: CollectionKind;
  itemCount: number;
  posterPath: string | null;
  updatedAt: string;
}

const KIND_LABELS: Record<CollectionKind, string> = {
  franchise: "Franchises",
  series: "Series",
  genre: "Genres",
};

/** Collections grouped for display: franchises, then series, then genres, largest first. */
export function groupCollections(list: CollectionSummary[]): { kind: CollectionKind; label: string; collections: CollectionSummary[] }[] {
  return (Object.keys(KIND_LABELS) as CollectionKind[])
    .map((kind) => ({
      kind,
      label: KIND_LABELS[kind],
      collections: list
        .filter((collection) => collection.kind === kind)
        .sort((a, b) => b.itemCount - a.itemCount || a.name.localeCompare(b.name)),
    }))
    .filter((group) => group.collections.length > 0);
}
