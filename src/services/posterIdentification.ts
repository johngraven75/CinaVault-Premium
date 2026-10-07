// Poster and film identification built on the local CLIP model.
//
// Business rules live in pure functions (normalizeSearchResults, titleSimilarity,
// scoreCandidates, decideIdentification, decidePosterVerification, ...) so they
// can be unit tested without a model or Tauri. The async wrappers at the bottom
// only fetch embeddings / call the backend and hand the numbers to those
// functions.

import { invoke } from "@tauri-apps/api/core";
import type { MediaItem } from "../store/appStore";
import { classifyImage, embedImage } from "./localVision.ts";
import {
  clamp01,
  cosineSimilarity,
  decideMatch,
  rankCandidates,
  type MatchDecisionOptions,
  type RankedCandidate,
} from "./visionMath.ts";

// ── Types ────────────────────────────────────────────────────────────────────

export type IdentifiableItem = Pick<MediaItem, "title" | "media_type"> &
  Partial<Pick<MediaItem, "id" | "year" | "poster_path" | "file_path">>;

/** One provider search hit, normalized across TMDb / OMDb / TVmaze / ThePornDB / Nuxt / PGMA. */
export interface MetadataCandidate {
  /** `<provider>:<provider id>` or `<provider>:#<index>` when the provider gave no id. */
  id: string;
  provider: string;
  title: string;
  year: number | null;
  posterUrl: string | null;
  mediaType: string | null;
  overview: string | null;
  tmdbId: string | null;
  imdbId: string | null;
}

export interface CandidateScore {
  candidate: MetadataCandidate;
  /** Combined score 0..1 used for ranking. */
  score: number;
  titleScore: number;
  yearScore: number | null;
  imageScore: number | null;
}

export interface IdentificationResult {
  best: CandidateScore | null;
  ranked: CandidateScore[];
  confidence: number;
  /** True when the best candidate is safe to apply without asking the user. */
  autoApply: boolean;
  reason: "no_candidates" | "below_min_score" | "ambiguous" | "matched";
}

export interface PosterVerification {
  ok: boolean;
  /** Probability (0..1) that the image is a poster for this title, vs the negatives. */
  score: number;
  /** Probability that the image is any kind of poster/cover art at all. */
  posterLikelihood: number;
}

export interface PosterPick {
  best: string | null;
  confidence: number;
  confident: boolean;
  ranked: RankedCandidate[];
}

// ── Tunables ─────────────────────────────────────────────────────────────────

export const SCORE_WEIGHTS = { title: 0.6, year: 0.15, image: 0.25 } as const;
/** Auto-apply only when the combined score and the lead over #2 are both clear. */
export const AUTO_APPLY_THRESHOLDS: MatchDecisionOptions = { minScore: 0.82, minMargin: 0.08 };
/** Below this the best candidate is not reported as a match at all. */
export const SUGGEST_THRESHOLDS: MatchDecisionOptions = { minScore: 0.55, minMargin: 0.02 };
/** CLIP image-to-image cosine: ~0.5 for unrelated posters, ~0.95+ for the same art. */
export const IMAGE_SIM_FLOOR = 0.5;
export const IMAGE_SIM_CEIL = 0.95;
export const POSTER_PICK_THRESHOLDS: MatchDecisionOptions = { minScore: 0.75, minMargin: 0.02 };
export const POSTER_VERIFY_MIN_SCORE = 0.35;
export const TMDB_IMAGE_BASE = "https://image.tmdb.org/t/p/w342";

const NEGATIVE_PROMPTS = [
  "a movie poster for a different film",
  "a photo of a person",
  "a screenshot from a video",
  "a blank or broken image",
  "a page of text",
  "a company logo",
] as const;
const NON_POSTER_PROMPTS = NEGATIVE_PROMPTS.slice(1);

// ── Pure helpers: titles and years ───────────────────────────────────────────

const LEADING_ARTICLE = /^(the|a|an)\s+/;

export function normalizeTitle(raw: string): string {
  return raw
    .normalize("NFKD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .replace(/,\s*(the|a|an)\b(?=\s*(\(|\[|$))/, "") // "Matrix, The (1999)"
    .replace(/\[[^\]]*\]|\([^)]*\)|\{[^}]*\}/g, " ") // [1080p] (2019) {tags}
    .replace(/&/g, " and ")
    .replace(/[._]/g, " ")
    .replace(/[^a-z0-9\s]/g, " ")
    .replace(/\b(19|20)\d{2}\b/g, " ")
    .replace(/\b(2160p|1080p|720p|480p|4k|uhd|hdr|bluray|web ?dl|webrip|x264|x265|hevc|remux)\b/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .replace(LEADING_ARTICLE, "");
}

function bigrams(value: string): string[] {
  const compact = value.replace(/\s+/g, " ");
  const grams: string[] = [];
  for (let index = 0; index < compact.length - 1; index += 1) grams.push(compact.slice(index, index + 2));
  return grams;
}

/** Sørensen–Dice over character bigrams. */
export function diceCoefficient(a: string, b: string): number {
  if (a === b) return a.length > 0 ? 1 : 0;
  const left = bigrams(a);
  const right = bigrams(b);
  if (left.length === 0 || right.length === 0) return 0;
  const counts = new Map<string, number>();
  for (const gram of left) counts.set(gram, (counts.get(gram) ?? 0) + 1);
  let overlap = 0;
  for (const gram of right) {
    const count = counts.get(gram) ?? 0;
    if (count > 0) {
      overlap += 1;
      counts.set(gram, count - 1);
    }
  }
  return (2 * overlap) / (left.length + right.length);
}

function tokenJaccard(a: string, b: string): number {
  const left = new Set(a.split(" ").filter(Boolean));
  const right = new Set(b.split(" ").filter(Boolean));
  if (left.size === 0 || right.size === 0) return 0;
  let shared = 0;
  for (const token of left) if (right.has(token)) shared += 1;
  return shared / (left.size + right.size - shared);
}

/** 0..1 similarity between two titles, robust to case, punctuation, articles and release tags. */
export function titleSimilarity(a: string, b: string): number {
  const left = normalizeTitle(a);
  const right = normalizeTitle(b);
  if (!left || !right) return 0;
  if (left === right) return 1;
  return Math.max(diceCoefficient(left, right), tokenJaccard(left, right));
}

export function yearSimilarity(a: number | null | undefined, b: number | null | undefined): number | null {
  if (!a || !b) return null;
  const diff = Math.abs(a - b);
  if (diff === 0) return 1;
  if (diff === 1) return 0.7; // festival vs. theatrical release year
  return 0;
}

export function parseYear(value: unknown): number | null {
  if (typeof value === "number" && value > 1800 && value < 3000) return Math.trunc(value);
  if (typeof value !== "string") return null;
  const match = value.match(/(18|19|20)\d{2}/);
  return match ? Number(match[0]) : null;
}

/** Maps CLIP image cosine similarity onto 0..1. */
export function imageSimilarityToScore(cosine: number): number {
  return clamp01((cosine - IMAGE_SIM_FLOOR) / (IMAGE_SIM_CEIL - IMAGE_SIM_FLOOR));
}

// ── Pure helpers: search_metadata result normalization ──────────────────────

function str(value: unknown): string | null {
  if (typeof value === "number" && Number.isFinite(value)) return String(value);
  if (typeof value !== "string") return null;
  const trimmed = value.trim();
  return trimmed && trimmed.toUpperCase() !== "N/A" ? trimmed : null;
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : null;
}

function posterFrom(entry: Record<string, unknown>): string | null {
  const direct =
    str(entry.poster_path) ?? str(entry.poster) ?? str(entry.Poster) ?? str(entry.image) ?? str(entry.cover);
  const images = Array.isArray(entry.images) ? asRecord(entry.images[0]) : null;
  const nested = asRecord(entry.image);
  const value = direct ?? str(images?.url) ?? str(nested?.original) ?? str(nested?.medium);
  if (!value) return null;
  if (/^(https?:|data:|asset:)/i.test(value)) return value;
  if (value.startsWith("/")) return `${TMDB_IMAGE_BASE}${value}`; // TMDb relative path
  return value;
}

/**
 * Turns the raw JSON returned by the `search_metadata` command into a flat
 * candidate list. Handles every response shape the backend currently returns:
 * TMDb `{results}`, OMDb `{Search}`, TVmaze `[{show}]`, ThePornDB `{data}`,
 * Porn Site Nuxt / PGMA `{results}`. Unknown shapes yield [].
 */
export function normalizeSearchResults(provider: string, raw: unknown): MetadataCandidate[] {
  const root = asRecord(raw);
  let entries: unknown[] = [];
  if (Array.isArray(raw)) entries = raw;
  else if (root) {
    const list = root.results ?? root.Search ?? root.data ?? root.scenes;
    if (Array.isArray(list)) entries = list;
  }

  const candidates: MetadataCandidate[] = [];
  entries.forEach((value, index) => {
    const outer = asRecord(value);
    if (!outer) return;
    const entry = asRecord(outer.show) ?? outer; // TVmaze wraps hits in {score, show}
    const title = str(entry.title) ?? str(entry.name) ?? str(entry.Title) ?? str(entry.original_title);
    if (!title) return;
    const tmdbMedia = str(entry.media_type);
    if (tmdbMedia === "person") return;
    const externals = asRecord(entry.externals);
    const imdbId = str(entry.imdbID) ?? str(entry.imdb_id) ?? str(externals?.imdb);
    const providerId = str(entry.id) ?? imdbId;
    const tmdbId = provider === "tmdb" ? str(entry.id) : str(entry.tmdb_id);
    candidates.push({
      id: `${provider}:${providerId ?? `#${index}`}`,
      provider,
      title,
      year: parseYear(
        entry.release_date ?? entry.first_air_date ?? entry.Year ?? entry.premiered ?? entry.year ?? entry.date,
      ),
      posterUrl: posterFrom(entry),
      mediaType: tmdbMedia ?? str(entry.Type) ?? str(entry.type) ?? null,
      overview: str(entry.overview) ?? str(entry.summary) ?? str(entry.details) ?? str(entry.description),
      tmdbId,
      imdbId,
    });
  });
  return candidates;
}

// ── Pure scoring / decisions ─────────────────────────────────────────────────

/**
 * Combines title, year and (optional) poster-image similarity into one score per
 * candidate and sorts best first. `imageCosines` maps candidate id -> raw CLIP
 * cosine between the item's reference image and the candidate's poster.
 * Signals that are unavailable are dropped and the remaining weights renormalized.
 */
export function scoreCandidates(
  item: IdentifiableItem,
  candidates: readonly MetadataCandidate[],
  imageCosines: ReadonlyMap<string, number> = new Map(),
): CandidateScore[] {
  const scored = candidates.map((candidate, order) => {
    const titleScore = titleSimilarity(item.title, candidate.title);
    const yearScore = yearSimilarity(item.year, candidate.year);
    const cosine = imageCosines.get(candidate.id);
    const imageScore = cosine === undefined ? null : imageSimilarityToScore(cosine);

    let weighted = SCORE_WEIGHTS.title * titleScore;
    let totalWeight: number = SCORE_WEIGHTS.title;
    if (yearScore !== null) {
      weighted += SCORE_WEIGHTS.year * yearScore;
      totalWeight += SCORE_WEIGHTS.year;
    }
    if (imageScore !== null) {
      weighted += SCORE_WEIGHTS.image * imageScore;
      totalWeight += SCORE_WEIGHTS.image;
    }
    return { order, result: { candidate, score: weighted / totalWeight, titleScore, yearScore, imageScore } };
  });
  return scored
    .sort((left, right) => right.result.score - left.result.score || left.order - right.order)
    .map((entry) => entry.result);
}

export function decideIdentification(
  ranked: readonly CandidateScore[],
  autoApply: MatchDecisionOptions = AUTO_APPLY_THRESHOLDS,
  suggest: MatchDecisionOptions = SUGGEST_THRESHOLDS,
): IdentificationResult {
  const asRanked = ranked.map((entry) => ({ id: entry.candidate.id, score: entry.score, data: entry }));
  const strict = decideMatch(asRanked, autoApply);
  if (strict.match) {
    return { best: strict.match.data ?? null, ranked: [...ranked], confidence: strict.confidence, autoApply: true, reason: "matched" };
  }
  const loose = decideMatch(asRanked, suggest);
  return {
    best: loose.match?.data ?? null,
    ranked: [...ranked],
    confidence: loose.match ? loose.confidence * 0.8 : 0,
    autoApply: false,
    reason: loose.reason === "matched" ? strict.reason : loose.reason,
  };
}

export function posterPrompt(item: Pick<IdentifiableItem, "title" | "media_type" | "year">): string {
  const title = item.title.trim();
  const year = item.year ? ` (${item.year})` : "";
  switch ((item.media_type ?? "").toLowerCase()) {
    case "tv":
    case "series":
    case "show":
    case "episode":
      return `a TV show poster for "${title}"${year}`;
    case "music":
    case "audio":
      return `an album cover for "${title}"`;
    case "adult":
      return `the cover art for the adult film "${title}"`;
    default:
      return `a movie poster for "${title}"${year}`;
  }
}

/** Labels passed to the zero-shot classifier: index 0 is the title prompt. */
export function posterVerificationLabels(item: Pick<IdentifiableItem, "title" | "media_type" | "year">): string[] {
  return [posterPrompt(item), ...NEGATIVE_PROMPTS];
}

/** Turns zero-shot probabilities (labels from posterVerificationLabels) into a verdict. */
export function decidePosterVerification(
  labels: readonly string[],
  probabilities: ReadonlyMap<string, number>,
  minScore = POSTER_VERIFY_MIN_SCORE,
): PosterVerification {
  const titleLabel = labels[0];
  const score = probabilities.get(titleLabel) ?? 0;
  const nonPoster = NON_POSTER_PROMPTS.reduce((sum, label) => sum + (probabilities.get(label) ?? 0), 0);
  const posterLikelihood = clamp01(1 - nonPoster);
  const differentFilm = probabilities.get(NEGATIVE_PROMPTS[0]) ?? 0;
  return { ok: score >= minScore && score >= differentFilm, score, posterLikelihood };
}

// ── Thin async wrappers (model + Tauri) ──────────────────────────────────────

/** Is this poster actually for this title? Uses CLIP zero-shot against generic negatives. */
export async function verifyPoster(
  item: Pick<IdentifiableItem, "title" | "media_type" | "year"> & { poster_path?: string | null },
  posterSrc: string | null = item.poster_path ?? null,
): Promise<PosterVerification> {
  if (!posterSrc || !item.title.trim()) return { ok: false, score: 0, posterLikelihood: 0 };
  const labels = posterVerificationLabels(item);
  const results = await classifyImage(posterSrc, labels);
  return decidePosterVerification(labels, new Map(results.map((entry) => [entry.label, entry.score])));
}

/** Chooses the candidate poster visually closest to a reference frame or poster. */
export async function pickBestPoster(
  referenceImageSrc: string,
  candidatePosterUrls: readonly string[],
): Promise<PosterPick> {
  const unique = [...new Set(candidatePosterUrls.filter(Boolean))];
  if (unique.length === 0) return { best: null, confidence: 0, confident: false, ranked: [] };
  const reference = await embedImage(referenceImageSrc);
  const embedded = await Promise.all(
    unique.map(async (url) => {
      try {
        return { id: url, embedding: await embedImage(url) };
      } catch {
        return null; // unreachable / undecodable image: skip it
      }
    }),
  );
  const ranked = rankCandidates(reference, embedded.filter((entry): entry is NonNullable<typeof entry> => entry !== null));
  const decision = decideMatch(ranked, POSTER_PICK_THRESHOLDS);
  return {
    best: ranked[0]?.id ?? null,
    confidence: decision.confidence,
    confident: decision.match !== null,
    ranked,
  };
}

/**
 * Ranks provider candidates for a library item by title, year and poster-image
 * similarity. `candidates` can be the raw `search_metadata` response or an
 * already normalized list. `referenceImageSrc` (an existing poster or a video
 * frame) enables the image signal; without it only title/year are used.
 */
export async function identifyFromCandidates(
  item: IdentifiableItem,
  candidates: readonly MetadataCandidate[] | unknown,
  options: { provider?: string; referenceImageSrc?: string | null; maxImageCandidates?: number } = {},
): Promise<IdentificationResult> {
  const list = Array.isArray(candidates) && candidates.every(isMetadataCandidate)
    ? (candidates as MetadataCandidate[])
    : normalizeSearchResults(options.provider ?? "unknown", candidates);

  const imageCosines = new Map<string, number>();
  const reference = options.referenceImageSrc ?? item.poster_path ?? null;
  if (reference && list.some((candidate) => candidate.posterUrl)) {
    try {
      const referenceEmbedding = await embedImage(reference);
      // Only spend vision time on the plausible top-N by title/year.
      const shortlist = scoreCandidates(item, list)
        .slice(0, options.maxImageCandidates ?? 8)
        .filter((entry) => entry.candidate.posterUrl);
      await Promise.all(
        shortlist.map(async ({ candidate }) => {
          try {
            const embedding = await embedImage(candidate.posterUrl as string);
            imageCosines.set(candidate.id, cosineSimilarity(referenceEmbedding, embedding));
          } catch {
            // poster failed to load: leave the image signal out for this candidate
          }
        }),
      );
    } catch (error) {
      console.warn("[posterIdentification] image signal unavailable:", error);
    }
  }
  return decideIdentification(scoreCandidates(item, list, imageCosines));
}

/** Convenience: query one provider through the existing backend command, then identify. */
export async function searchAndIdentify(
  item: IdentifiableItem,
  provider: string,
  options: { apiKey?: string | null; referenceImageSrc?: string | null } = {},
): Promise<IdentificationResult> {
  const raw = await invoke<unknown>("search_metadata", {
    provider,
    query: item.title,
    mediaType: item.media_type ?? null,
    apiKey: options.apiKey ?? null,
  });
  return identifyFromCandidates(item, raw, { provider, referenceImageSrc: options.referenceImageSrc });
}

function isMetadataCandidate(value: unknown): value is MetadataCandidate {
  const record = asRecord(value);
  return !!record && typeof record.id === "string" && typeof record.provider === "string" && typeof record.title === "string" && "posterUrl" in record;
}
