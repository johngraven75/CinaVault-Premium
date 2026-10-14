// Live TV helpers shared by the embedded player and the programme guide.

export type StreamKind = "hls" | "mpegts" | "native";

export interface GuideProgramme {
  start: number; // Unix seconds
  stop: number;
  title: string;
  sub_title?: string | null;
  description?: string | null;
  category?: string | null;
  episode?: string | null;
}

const NATIVE_EXTENSIONS = [".mp4", ".m4v", ".webm", ".mov", ".mp3", ".aac", ".m4a", ".ogg"];

/**
 * Picks the engine for a stream URL: hls.js for HLS playlists, mpegts.js for
 * raw MPEG-TS (Xtream `/live/.../id.ts` and extensionless live paths), and
 * the browser's own decoder for plain files.
 */
export function detectStreamKind(url: string): StreamKind {
  let path = url.trim().toLowerCase();
  try {
    const parsed = new URL(url.trim());
    path = parsed.pathname.toLowerCase();
    const output = parsed.searchParams.get("output")?.toLowerCase();
    if (output === "m3u8" || output === "hls") return "hls";
    if (output === "ts" || output === "mpegts") return "mpegts";
  } catch {
    path = path.split(/[?#]/)[0];
  }
  if (path.endsWith(".m3u8") || path.endsWith(".m3u")) return "hls";
  if (NATIVE_EXTENSIONS.some((extension) => path.endsWith(extension))) return "native";
  // .ts, .mpegts, .flv-less live endpoints and extensionless Xtream paths.
  return "mpegts";
}

export function isPlayableUrl(url: string): boolean {
  try {
    const parsed = new URL(url.trim());
    return (parsed.protocol === "http:" || parsed.protocol === "https:") && !!parsed.hostname;
  } catch {
    return false;
  }
}

/** Share of the programme already aired, clamped to 0..1. */
export function programmeProgress(programme: Pick<GuideProgramme, "start" | "stop">, nowSeconds: number): number {
  const length = programme.stop - programme.start;
  if (length <= 0) return 0;
  return Math.min(1, Math.max(0, (nowSeconds - programme.start) / length));
}

/** Splits a guide into what is on now and what comes next. */
export function splitGuide(programmes: GuideProgramme[], nowSeconds: number) {
  const sorted = [...programmes].sort((a, b) => a.start - b.start);
  // When guides overlap, the programme that started most recently is the
  // current one; an earlier long block is usually a stale or coarser listing.
  const active = sorted.filter((item) => item.start <= nowSeconds && item.stop > nowSeconds);
  const current = active.length ? active[active.length - 1] : null;
  const after = current ? current.stop : nowSeconds;
  const upcoming = sorted.filter((item) => item !== current && item.start >= after);
  return { current, upcoming };
}

export function formatGuideTime(seconds: number, locale?: string): string {
  return new Date(seconds * 1000).toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" });
}

/** "45 min", "1 h 30 min". */
export function formatDuration(seconds: number): string {
  const minutes = Math.max(0, Math.round(seconds / 60));
  if (minutes < 60) return `${minutes} min`;
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return rest ? `${hours} h ${rest} min` : `${hours} h`;
}

/** "Ends in 12 min" style remaining-time label for the current programme. */
export function remainingLabel(programme: Pick<GuideProgramme, "stop">, nowSeconds: number): string {
  const left = programme.stop - nowSeconds;
  return left <= 60 ? "Ending now" : `${formatDuration(left)} left`;
}

/** A day label when a programme is not today ("Tomorrow", "Wed"). */
export function dayLabel(seconds: number, nowSeconds: number, locale?: string): string | null {
  const day = new Date(seconds * 1000);
  const today = new Date(nowSeconds * 1000);
  const startOfDay = (date: Date) => new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
  const diffDays = Math.round((startOfDay(day) - startOfDay(today)) / 86_400_000);
  if (diffDays === 0) return null;
  if (diffDays === 1) return "Tomorrow";
  return day.toLocaleDateString(locale, { weekday: "short" });
}

export interface M3uPlaylistForm {
  name: string;
  url: string;
}

export function buildAddM3uArgs(form: M3uPlaylistForm): { name: string; url: string } {
  const url = form.url.trim();
  if (!isPlayableUrl(url)) {
    throw new Error("Enter the full playlist URL, for example http://provider.com/get.php?...&type=m3u_plus.");
  }
  return { name: form.name.trim() || "M3U playlist", url };
}
