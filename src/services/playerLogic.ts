// Decisions the built-in player makes, kept free of React, Tauri and the DOM
// so Node tests can cover them: how to play a file (direct, transcode or the
// external player), skip windows, Next Up timing, resume rules, buffering and
// fades. LibraryPlayer wires these to the <video> element.

export interface Chapter {
  start: number;
  end: number;
  title: string;
}

/** ffprobe summary returned by player_prepare (transcode.rs MediaProbe). */
export interface ProbeInfo {
  duration: number;
  formatName: string;
  videoCodec?: string | null;
  videoProfile?: string | null;
  pixFmt?: string | null;
  width?: number | null;
  height?: number | null;
  audioCodec?: string | null;
  chapters: Chapter[];
}

export type Route =
  | { kind: "direct" }
  | { kind: "transcode" }
  | { kind: "external"; reason: string };

// ── Direct play or transcode ────────────────────────────────────────────────

export function fileExtension(path: string): string {
  const name = path.split(/[\\/]/).pop() ?? "";
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

const CONTAINER_MIME: Record<string, string> = {
  mp4: "video/mp4",
  m4v: "video/mp4",
  mov: "video/mp4",
  webm: "video/webm",
  mkv: "video/x-matroska",
};

const VIDEO_CODECS: Record<string, string> = {
  h264: "avc1.640028",
  hevc: "hvc1.1.6.L120.90",
  vp9: "vp09.00.40.08",
  vp8: "vp8",
  av1: "av01.0.08M.08",
};

const AUDIO_CODECS: Record<string, string> = {
  aac: "mp4a.40.2",
  mp3: "mp4a.69",
  opus: "opus",
  vorbis: "vorbis",
  flac: "flac",
  ac3: "ac-3",
  eac3: "ec-3",
};

/** Browsers decode only 8-bit H.264; High 10 and 4:2:2/4:4:4 need a transcode. */
function unsupportedH264(probe: ProbeInfo): boolean {
  if (probe.videoCodec !== "h264") return false;
  const profile = (probe.videoProfile ?? "").toLowerCase();
  const pix = (probe.pixFmt ?? "").toLowerCase();
  return /10|422|444/.test(profile) || /10|12|422|444/.test(pix);
}

/**
 * The MIME type (with codecs when known) to ask canPlayType about, or null
 * when the container or a codec is one the WebView never decodes.
 */
export function directPlayMime(filePath: string, probe: ProbeInfo | null): string | null {
  const container = CONTAINER_MIME[fileExtension(filePath)];
  if (!container) return null;
  if (!probe) return container;
  const codecs: string[] = [];
  if (probe.videoCodec) {
    const video = VIDEO_CODECS[probe.videoCodec];
    if (!video || unsupportedH264(probe)) return null;
    codecs.push(video);
  }
  if (probe.audioCodec) {
    const audio = AUDIO_CODECS[probe.audioCodec];
    if (!audio) return null;
    codecs.push(audio);
  }
  return codecs.length ? `${container}; codecs="${codecs.join(", ")}"` : container;
}

export interface RouteInput {
  filePath: string;
  probe: ProbeInfo | null;
  /** HTMLMediaElement.canPlayType, injected for tests. */
  canPlayType: (mime: string) => string;
  forceDirectPlay: boolean;
  transcodeAvailable: boolean;
}

function describeFormat(filePath: string, probe: ProbeInfo | null): string {
  const parts = [fileExtension(filePath).toUpperCase() || "unknown container"];
  if (probe?.videoCodec) parts.push(probe.videoCodec.toUpperCase());
  if (probe?.audioCodec) parts.push(probe.audioCodec.toUpperCase());
  return parts.join(" / ");
}

function cannotPlayInApp(input: Omit<RouteInput, "canPlayType">): Route {
  const format = describeFormat(input.filePath, input.probe);
  if (input.forceDirectPlay) {
    return {
      kind: "external",
      reason: `Force Direct Play is on and the app can't decode ${format}, so it opened in your external player.`,
    };
  }
  if (input.transcodeAvailable) return { kind: "transcode" };
  return {
    kind: "external",
    reason: `The app can't decode ${format} and ffmpeg isn't available to convert it, so it opened in your external player.`,
  };
}

/** Plays the original in the WebView when it can decode it, else transcodes or hands off. */
export function chooseRoute(input: RouteInput): Route {
  const mime = directPlayMime(input.filePath, input.probe);
  if (mime && input.canPlayType(mime) !== "") return { kind: "direct" };
  return cannotPlayInApp(input);
}

/** What to try after the <video> element reports an error on `current`. */
export function routeAfterError(
  current: Route,
  input: Omit<RouteInput, "canPlayType">,
): Route {
  if (current.kind === "direct") return cannotPlayInApp(input);
  return {
    kind: "external",
    reason: `The built-in player couldn't play ${describeFormat(input.filePath, input.probe)}, so it opened in your external player.`,
  };
}

/** Transcode URL that starts at `start` seconds (seeking restarts the transcode). */
export function transcodeSrc(url: string, start: number): string {
  const at = Number.isFinite(start) && start > 0 ? start : 0;
  return at > 0 ? `${url}?start=${at.toFixed(3)}` : url;
}

// ── Skip intro / credits ────────────────────────────────────────────────────

export interface SkipWindow {
  start: number;
  end: number;
  source: "chapter" | "window";
}

export interface SkipWindows {
  intro: SkipWindow | null;
  credits: SkipWindow | null;
}

export interface SkipSettings {
  introEnabled: boolean;
  creditsEnabled: boolean;
  /** Fallback intro length when the file has no intro chapter (0 = none). */
  introSeconds: number;
  /** Fallback credits length when the file has no credits chapter (0 = none). */
  creditsSeconds: number;
}

const INTRO_NAME = /\b(intro|opening|op)\b|^op\s*\d+$/i;
const CREDITS_NAME = /\b(credits|ending|outro|ed)\b|^ed\s*\d+$/i;

/** Files shorter than this get no fallback windows (clips, trailers). */
export const MIN_FALLBACK_DURATION = 600;

export function chapterKind(title: string): "intro" | "credits" | null {
  if (INTRO_NAME.test(title)) return "intro";
  if (CREDITS_NAME.test(title)) return "credits";
  return null;
}

export function skipWindows(chapters: Chapter[], duration: number, settings: SkipSettings): SkipWindows {
  const half = duration / 2;
  const introChapter = chapters.find((c) => chapterKind(c.title) === "intro" && (duration <= 0 || c.start < half));
  const creditsChapter = [...chapters]
    .reverse()
    .find((c) => chapterKind(c.title) === "credits" && (duration <= 0 || c.start >= half));
  const fallbackOk = duration >= MIN_FALLBACK_DURATION;

  let intro: SkipWindow | null = null;
  if (settings.introEnabled) {
    if (introChapter) intro = { start: introChapter.start, end: introChapter.end, source: "chapter" };
    else if (fallbackOk && settings.introSeconds > 0)
      intro = { start: 0, end: Math.min(settings.introSeconds, duration / 4), source: "window" };
  }
  let credits: SkipWindow | null = null;
  if (settings.creditsEnabled) {
    if (creditsChapter)
      credits = { start: creditsChapter.start, end: Math.min(creditsChapter.end, duration || creditsChapter.end), source: "chapter" };
    else if (fallbackOk && settings.creditsSeconds > 0)
      credits = { start: duration - Math.min(settings.creditsSeconds, duration / 4), end: duration, source: "window" };
  }
  return { intro, credits };
}

/** Which Skip button to show at `time`. Hidden in the last second of a window. */
export function activeSkip(time: number, windows: SkipWindows): "intro" | "credits" | null {
  const inside = (w: SkipWindow | null) => !!w && time >= w.start && time < w.end - 1;
  if (inside(windows.intro)) return "intro";
  if (inside(windows.credits)) return "credits";
  return null;
}

// ── Next Up ─────────────────────────────────────────────────────────────────

export interface NextUpSettings {
  countdownSeconds: number;
  secondsBeforeEnd: number;
}

/** When the Next Up card appears: credits start if known, else N seconds before the end. */
export function nextUpStart(duration: number, credits: SkipWindow | null, settings: NextUpSettings): number | null {
  if (!(duration > 0)) return null;
  if (credits && credits.start > 0 && credits.start < duration) return credits.start;
  const lead = Math.min(Math.max(settings.secondsBeforeEnd, settings.countdownSeconds), duration / 4);
  return Math.max(0, duration - lead);
}

/** Seconds left on the countdown, counted in playback time so pausing pauses it. */
export function nextUpRemaining(time: number, shownAt: number, countdownSeconds: number): number {
  return Math.max(0, Math.ceil(countdownSeconds - Math.max(0, time - shownAt)));
}

/** Gapless preloads the next title once this close to the end. */
export const GAPLESS_PRELOAD_SECONDS = 90;

export function shouldPreloadNext(time: number, duration: number, nextUpAt: number | null): boolean {
  if (!(duration > 0)) return false;
  return duration - time <= GAPLESS_PRELOAD_SECONDS || (nextUpAt !== null && time >= nextUpAt - 30);
}

// ── Auto resume ─────────────────────────────────────────────────────────────

export interface SavedProgress {
  position: number;
  duration: number;
  finished: boolean;
}

export const RESUME_MIN_SECONDS = 15;
export const PROGRESS_SAVE_MS = 10_000;

export interface ResumeDecision {
  position: number;
  reason: "requested" | "resume" | "start";
}

/** Where to start: an explicit startAt wins, then saved progress unless finished or under 15 s. */
export function resumeDecision(
  progress: SavedProgress | null,
  autoResume: boolean,
  startAt?: number,
): ResumeDecision {
  if (typeof startAt === "number" && Number.isFinite(startAt) && startAt >= 0) {
    return { position: startAt, reason: "requested" };
  }
  if (!autoResume || !progress || progress.finished) return { position: 0, reason: "start" };
  const { position, duration } = progress;
  if (!(position >= RESUME_MIN_SECONDS)) return { position: 0, reason: "start" };
  if (duration > 0 && position >= duration - 5) return { position: 0, reason: "start" };
  return { position, reason: "resume" };
}

export function shouldSaveProgress(lastSavedAtMs: number, nowMs: number, lastPosition: number, position: number): boolean {
  return nowMs - lastSavedAtMs >= PROGRESS_SAVE_MS && Math.abs(position - lastPosition) >= 1;
}

// ── Buffering ───────────────────────────────────────────────────────────────

export interface BufferSettings {
  bufferAheadSeconds?: number;
  chunkKb?: number;
}

export interface BufferPlan {
  preload: "auto" | "metadata";
  /** Seconds to have buffered before resuming after a stall; 0 = browser default. */
  bufferAheadSeconds: number;
  chunkKb: number;
}

const clamp = (value: unknown, min: number, max: number, fallback: number): number => {
  const n = typeof value === "string" ? Number(value) : value;
  return typeof n === "number" && Number.isFinite(n) ? Math.min(max, Math.max(min, n)) : fallback;
};

/** Mirrors transcode.rs buffer_plan so both sides agree. */
export function bufferPlan(enabled: boolean, config: BufferSettings): BufferPlan {
  if (!enabled) return { preload: "metadata", bufferAheadSeconds: 0, chunkKb: 64 };
  return {
    preload: "auto",
    bufferAheadSeconds: Math.round(clamp(config.bufferAheadSeconds, 5, 300, 30)),
    chunkKb: Math.round(clamp(config.chunkKb, 16, 4096, 256)),
  };
}

/** Seconds buffered contiguously after `time`, from TimeRanges-like pairs. */
export function bufferedAhead(ranges: Array<[number, number]>, time: number): number {
  for (const [start, end] of ranges) {
    if (time >= start - 0.25 && time <= end) return Math.max(0, end - time);
  }
  return 0;
}

/** Longest a stall is held for the buffer target before playing anyway. */
export const MAX_BUFFER_HOLD_MS = 15_000;

/**
 * After a stall: keep holding until the buffer target (or the end) is reached.
 * `loading` is false once the browser has stopped fetching (it suspends paused
 * media), when waiting longer would gain nothing.
 */
export function bufferHold(
  ahead: number,
  target: number,
  remaining: number,
  heldMs: number,
  loading = true,
): "hold" | "resume" {
  if (target <= 0 || heldMs >= MAX_BUFFER_HOLD_MS || !loading) return "resume";
  return ahead >= Math.min(target, Math.max(0, remaining - 1)) ? "resume" : "hold";
}

// ── Crossfade ───────────────────────────────────────────────────────────────

export function fadeSeconds(enabled: boolean, config: { seconds?: number }): number {
  return enabled ? clamp(config.seconds, 0.2, 10, 1.5) : 0;
}

// ── Display ─────────────────────────────────────────────────────────────────

export function formatClock(seconds: number): string {
  const total = Math.max(0, Math.floor(Number.isFinite(seconds) ? seconds : 0));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = String(total % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
}
