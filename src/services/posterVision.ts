// Vision skill: reads a poster (or any still image) with free Hugging Face
// models and turns it into library metadata.
//
//   1. Local CLIP (Xenova/clip-vit-base-patch32, see localVision.ts) always
//      runs: bundled with the installer, offline, no key. Zero-shot prompts
//      give the kind of artwork, genres, mood, era and content tags.
//   2. When a Hugging Face token is set, Qwen3-VL (open weights, Apache-2.0)
//      reads the printed text through the Hugging Face router, which the free
//      HF account's monthly credits cover: title, year, tagline and a caption.
//
// The scoring rules below are pure functions so they are testable without the
// model; analyzePoster/readPosterText take their model calls as parameters.

import { invoke } from "@tauri-apps/api/core";
import { classifyImage, VISION_MODEL_ID, type ImageSource, type LabelScore } from "./localVision.ts";
import { titleSimilarity } from "./posterIdentification.ts";

/** Open-weights vision-language model reached through router.huggingface.co. */
export const POSTER_READ_MODEL = "Qwen/Qwen3-VL-8B-Instruct";

export type Classifier = (src: ImageSource, labels: readonly string[]) => Promise<LabelScore[]>;

export const KIND_PROMPTS = {
  movie: "a movie poster",
  series: "a TV series poster",
  animation: "a poster for an animated cartoon",
  anime: "an anime poster",
  documentary: "a documentary poster",
  music: "a concert or music album poster",
  screenshot: "a screenshot of a video frame",
  photo: "an ordinary photo that is not a poster",
} as const;
export type PosterKind = keyof typeof KIND_PROMPTS;
const NON_POSTER_KINDS: ReadonlySet<PosterKind> = new Set(["screenshot", "photo"]);

/** TMDB genre names, so filled-in genres line up with provider metadata. */
export const GENRES = [
  "Action",
  "Adventure",
  "Animation",
  "Comedy",
  "Crime",
  "Documentary",
  "Drama",
  "Family",
  "Fantasy",
  "History",
  "Horror",
  "Music",
  "Mystery",
  "Romance",
  "Science Fiction",
  "Thriller",
  "War",
  "Western",
] as const;

export const MOOD_PROMPTS = {
  dark: "a dark and moody poster",
  bright: "a bright and colorful poster",
  tense: "a tense and suspenseful poster",
  playful: "a playful and lighthearted poster",
  romantic: "a romantic and tender poster",
  epic: "an epic and grand poster",
} as const;

export const ERA_PROMPTS = {
  "pre-1960s": "a vintage poster from the 1940s or 1950s",
  "1960s-70s": "a retro poster from the 1960s or 1970s",
  "1980s": "a poster from the 1980s",
  "1990s": "a poster from the 1990s",
  modern: "a modern digital poster from the 2010s or 2020s",
} as const;

/** Each tag is tested on its own against a neutral prompt (multi-label). */
export const TAG_PROMPTS = {
  explosions: "a poster with explosions and fire",
  guns: "a poster with people holding guns",
  spaceships: "a poster with spaceships",
  monsters: "a poster with a monster or creature",
  superheroes: "a poster with superheroes in costumes",
  cars: "a poster with fast cars",
  horses: "a poster with horses and cowboys",
  soldiers: "a poster with soldiers in battle",
  couple: "a poster with a romantic couple",
  children: "a poster with children",
  animals: "a poster with animals",
  city: "a poster with a city skyline",
  ocean: "a poster with the ocean or sea",
  "outer space": "a poster with planets and outer space",
  robots: "a poster with robots",
  magic: "a poster with magic and wizards",
  blood: "a poster with blood",
} as const;
export const TAG_NEUTRAL_PROMPT = "a poster";

export const THRESHOLDS = {
  /** Kind needs this probability to be reported. */
  kind: 0.35,
  /** A genre is kept at this probability, or at half the top genre's when that is higher. */
  genre: 0.18,
  genreRelative: 0.5,
  maxGenres: 3,
  mood: 0.35,
  era: 0.4,
  /** A tag beats the neutral prompt with at least this probability. */
  tag: 0.7,
  maxTags: 6,
} as const;

export interface PosterInsights {
  isPoster: boolean;
  kind: PosterKind | null;
  genres: string[];
  moods: string[];
  era: string | null;
  tags: string[];
  /** Probability of the top genre: how sure the genre guess is. */
  confidence: number;
}

export interface PosterScores {
  kind: LabelScore[];
  genre: LabelScore[];
  mood: LabelScore[];
  era: LabelScore[];
  /** Per tag: probability it beats the neutral prompt. */
  tags: LabelScore[];
}

export interface PosterRead {
  title: string | null;
  year: number | null;
  tagline: string | null;
  caption: string | null;
}

export function genrePrompt(genre: string): string {
  return `a poster for a ${genre.toLowerCase()} film`;
}

function keyFor<T extends Record<string, string>>(prompts: T, prompt: string): keyof T | undefined {
  return (Object.keys(prompts) as (keyof T)[]).find((key) => prompts[key] === prompt);
}

/** Turns raw CLIP scores (already mapped to keys) into insights. */
export function insightsFromScores(scores: PosterScores): PosterInsights {
  const sorted = (list: LabelScore[]) => [...list].sort((a, b) => b.score - a.score);
  const kindTop = sorted(scores.kind)[0];
  const kind = kindTop && kindTop.score >= THRESHOLDS.kind ? (kindTop.label as PosterKind) : null;
  const isPoster = !kindTop || !NON_POSTER_KINDS.has(kindTop.label as PosterKind);

  const genresSorted = sorted(scores.genre);
  const topGenre = genresSorted[0]?.score ?? 0;
  const genreFloor = Math.max(THRESHOLDS.genre, topGenre * THRESHOLDS.genreRelative);
  const genres = genresSorted
    .filter((entry) => entry.score >= genreFloor)
    .slice(0, THRESHOLDS.maxGenres)
    .map((entry) => entry.label);

  const moods = sorted(scores.mood)
    .filter((entry) => entry.score >= THRESHOLDS.mood)
    .slice(0, 2)
    .map((entry) => entry.label);
  const eraTop = sorted(scores.era)[0];
  const era = eraTop && eraTop.score >= THRESHOLDS.era ? eraTop.label : null;
  const tags = sorted(scores.tags)
    .filter((entry) => entry.score >= THRESHOLDS.tag)
    .slice(0, THRESHOLDS.maxTags)
    .map((entry) => entry.label);

  return {
    isPoster,
    kind,
    genres: isPoster ? genres : [],
    moods,
    era: isPoster ? era : null,
    tags,
    confidence: isPoster ? topGenre : 0,
  };
}

function mapGroup<T extends Record<string, string>>(prompts: T, results: LabelScore[]): LabelScore[] {
  return results.flatMap((entry) => {
    const key = keyFor(prompts, entry.label);
    return key === undefined ? [] : [{ label: String(key), score: entry.score }];
  });
}

/** Runs every CLIP prompt group on one image. The image embedding is computed once and cached. */
export async function scorePoster(src: ImageSource, classify: Classifier = classifyImage): Promise<PosterScores> {
  const genrePrompts = Object.fromEntries(GENRES.map((genre) => [genre, genrePrompt(genre)]));
  const [kind, genre, mood, era] = await Promise.all([
    classify(src, Object.values(KIND_PROMPTS)),
    classify(src, Object.values(genrePrompts)),
    classify(src, Object.values(MOOD_PROMPTS)),
    classify(src, Object.values(ERA_PROMPTS)),
  ]);
  const tags = await Promise.all(
    (Object.keys(TAG_PROMPTS) as (keyof typeof TAG_PROMPTS)[]).map(async (tag) => {
      const pair = await classify(src, [TAG_PROMPTS[tag], TAG_NEUTRAL_PROMPT]);
      const hit = pair.find((entry) => entry.label === TAG_PROMPTS[tag]);
      return { label: tag, score: hit?.score ?? 0 };
    }),
  );
  return {
    kind: mapGroup(KIND_PROMPTS, kind),
    genre: mapGroup(genrePrompts, genre),
    mood: mapGroup(MOOD_PROMPTS, mood),
    era: mapGroup(ERA_PROMPTS, era),
    tags,
  };
}

export async function analyzePoster(src: ImageSource, classify: Classifier = classifyImage): Promise<PosterInsights> {
  return insightsFromScores(await scorePoster(src, classify));
}

// ── Cloud read (Hugging Face router, optional) ───────────────────────────────

export const POSTER_READ_PROMPT = [
  "Look at this image. If it is a movie or TV poster or cover, read its printed text.",
  'Reply with only JSON: {"title": string|null, "year": number|null, "tagline": string|null, "caption": string}.',
  "title and year must be printed on the image, otherwise null. caption is one short sentence describing what is shown.",
].join(" ");

function cleanString(value: unknown, max: number): string | null {
  if (typeof value !== "string") return null;
  const text = value.replace(/\s+/g, " ").trim();
  return text && text.toLowerCase() !== "null" ? text.slice(0, max) : null;
}

/** Parses the model's reply; tolerates code fences and extra prose around the JSON. */
export function parsePosterRead(reply: unknown, now = new Date()): PosterRead | null {
  if (typeof reply !== "string") return null;
  const start = reply.indexOf("{");
  const end = reply.lastIndexOf("}");
  if (start < 0 || end <= start) return null;
  let data: Record<string, unknown>;
  try {
    data = JSON.parse(reply.slice(start, end + 1)) as Record<string, unknown>;
  } catch {
    return null;
  }
  if (!data || typeof data !== "object") return null;
  const rawYear = typeof data.year === "string" ? Number.parseInt(data.year, 10) : data.year;
  const year =
    typeof rawYear === "number" && Number.isInteger(rawYear) && rawYear >= 1880 && rawYear <= now.getFullYear() + 2
      ? rawYear
      : null;
  return {
    title: cleanString(data.title, 200),
    year,
    tagline: cleanString(data.tagline, 200),
    caption: cleanString(data.caption, 300),
  };
}

export interface InferenceResult {
  status?: string;
  message?: unknown;
}
export type InferenceCall = (args: { input: string; model: string; imageUrl: string }) => Promise<InferenceResult>;

const defaultInference: InferenceCall = (args) => invoke<InferenceResult>("ai_inference", args);

/** True when the free Hugging Face path can be used (a token is configured). */
export async function hasHuggingFaceToken(): Promise<boolean> {
  const config = await invoke<{ has_token?: boolean }>("get_ai_config").catch(() => null);
  return config?.has_token === true;
}

/**
 * Reads printed text and a caption with Qwen3-VL. `imageUrl` must be an
 * https URL or a data: URL. Resolves to null on any failure.
 */
export async function readPosterText(
  imageUrl: string,
  infer: InferenceCall = defaultInference,
  question: string = POSTER_READ_PROMPT,
): Promise<PosterRead | null> {
  try {
    const result = await infer({ input: question, model: POSTER_READ_MODEL, imageUrl });
    if (result?.status !== "success") return null;
    return parsePosterRead(result.message);
  } catch {
    return null;
  }
}

// ── Turning insights into library fields and words ──────────────────────────

/** Genre text for `media_items.genre`; null when the guess is too weak. */
export function genreField(insights: PosterInsights): string | null {
  return insights.isPoster && insights.genres.length > 0 ? insights.genres.join(", ") : null;
}

/** Year worth filling in: only when the printed title matches the library title. */
export function yearToFill(itemTitle: string, itemYear: number | null | undefined, read: PosterRead | null): number | null {
  if (itemYear || !read?.year || !read.title) return null;
  return titleSimilarity(itemTitle, read.title) >= 0.8 ? read.year : null;
}

const KIND_WORDS: Record<PosterKind, string> = {
  movie: "a movie poster",
  series: "a TV series poster",
  animation: "poster art for an animated title",
  anime: "anime poster art",
  documentary: "a documentary poster",
  music: "a music or concert poster",
  screenshot: "a frame from a video",
  photo: "a photo rather than a poster",
};

function list(words: readonly string[]): string {
  if (words.length <= 1) return words.join("");
  return `${words.slice(0, -1).join(", ")} and ${words[words.length - 1]}`;
}

/** One or two plain sentences for the agent to say. */
export function describePoster(insights: PosterInsights, read: PosterRead | null = null): string {
  const parts: string[] = [];
  if (read?.title) {
    parts.push(`It reads "${read.title}"${read.year ? ` (${read.year})` : ""}${read.tagline ? `, with the tagline "${read.tagline}"` : ""}.`);
  }
  if (read?.caption) parts.push(read.caption.endsWith(".") ? read.caption : `${read.caption}.`);
  const looks = insights.kind ? KIND_WORDS[insights.kind] : insights.isPoster ? "poster art" : "an image";
  const genre = insights.genres.length ? ` for ${list(insights.genres.map((g) => g.toLowerCase()))}` : "";
  const mood = insights.moods.length ? `, ${list(insights.moods)} in tone` : "";
  const era = insights.era ? `, styled like ${insights.era === "modern" ? "a recent release" : `the ${insights.era}`}` : "";
  parts.push(`To me it looks like ${looks}${genre}${mood}${era}.`);
  if (insights.tags.length) parts.push(`I can see ${list(insights.tags)}.`);
  return parts.join(" ");
}

export const LOCAL_MODEL_LABEL = VISION_MODEL_ID;
