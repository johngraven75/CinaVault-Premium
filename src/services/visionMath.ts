// Pure vector math for the local AI vision layer.
// No imports from @huggingface/transformers so this module is cheap to load
// and can be unit tested directly under node:test.

export type Vector = ArrayLike<number>;

export interface EmbeddingCandidate<T = unknown> {
  id: string;
  embedding: Vector;
  data?: T;
}

export interface RankedCandidate<T = unknown> {
  id: string;
  score: number;
  data?: T;
}

export interface MatchDecisionOptions {
  /** Best score must be at least this to count as a match. */
  minScore: number;
  /** Best score must beat the runner-up by at least this much. */
  minMargin: number;
}

export interface MatchDecision<T = unknown> {
  match: RankedCandidate<T> | null;
  /** 0..1, how far the winner clears both thresholds. 0 when there is no match. */
  confidence: number;
  reason: "no_candidates" | "below_min_score" | "ambiguous" | "matched";
}

export function dot(a: Vector, b: Vector): number {
  const length = Math.min(a.length, b.length);
  let sum = 0;
  for (let index = 0; index < length; index += 1) sum += a[index] * b[index];
  return sum;
}

export function norm(vector: Vector): number {
  return Math.sqrt(dot(vector, vector));
}

/** Returns a unit-length copy. A zero vector stays all zeros. */
export function normalize(vector: Vector): Float32Array {
  const out = new Float32Array(vector.length);
  const length = norm(vector);
  if (!Number.isFinite(length) || length === 0) return out;
  for (let index = 0; index < vector.length; index += 1) out[index] = vector[index] / length;
  return out;
}

/** Cosine similarity in [-1, 1]. Mismatched lengths or zero vectors return 0. */
export function cosineSimilarity(a: Vector, b: Vector): number {
  if (a.length === 0 || a.length !== b.length) return 0;
  const denominator = norm(a) * norm(b);
  if (!Number.isFinite(denominator) || denominator === 0) return 0;
  const value = dot(a, b) / denominator;
  return Math.max(-1, Math.min(1, value));
}

/** Numerically stable softmax. `scale` matches CLIP's logit scale (100). */
export function softmax(values: readonly number[], scale = 1): number[] {
  if (values.length === 0) return [];
  const scaled = values.map((value) => value * scale);
  const max = Math.max(...scaled);
  const exps = scaled.map((value) => Math.exp(value - max));
  const total = exps.reduce((sum, value) => sum + value, 0);
  return exps.map((value) => value / total);
}

/** Element-wise mean of several vectors, normalized. Used to build prompt ensembles. */
export function meanEmbedding(vectors: readonly Vector[]): Float32Array {
  if (vectors.length === 0) return new Float32Array(0);
  const size = vectors[0].length;
  const sum = new Float32Array(size);
  for (const vector of vectors) {
    if (vector.length !== size) continue;
    for (let index = 0; index < size; index += 1) sum[index] += vector[index];
  }
  return normalize(sum);
}

/** Scores every candidate against the query and sorts best first. Ties keep input order. */
export function rankCandidates<T>(
  queryEmbedding: Vector,
  candidates: readonly EmbeddingCandidate<T>[],
): RankedCandidate<T>[] {
  return candidates
    .map((candidate, order) => ({
      order,
      ranked: {
        id: candidate.id,
        score: cosineSimilarity(queryEmbedding, candidate.embedding),
        ...(candidate.data === undefined ? {} : { data: candidate.data }),
      } as RankedCandidate<T>,
    }))
    .sort((left, right) => right.ranked.score - left.ranked.score || left.order - right.order)
    .map((entry) => entry.ranked);
}

/** Decides whether the top-ranked candidate is a confident, unambiguous match. */
export function decideMatch<T>(
  ranked: readonly RankedCandidate<T>[],
  options: MatchDecisionOptions,
): MatchDecision<T> {
  const [best, runnerUp] = ranked;
  if (!best) return { match: null, confidence: 0, reason: "no_candidates" };
  if (!(best.score >= options.minScore)) {
    return { match: null, confidence: 0, reason: "below_min_score" };
  }
  const margin = runnerUp ? best.score - runnerUp.score : Number.POSITIVE_INFINITY;
  if (margin < options.minMargin) return { match: null, confidence: 0, reason: "ambiguous" };

  // Confidence: half from how far above minScore we are (relative to the room
  // left up to 1.0), half from how far the margin exceeds minMargin.
  const scoreRoom = Math.max(1 - options.minScore, 1e-6);
  const scorePart = clamp01((best.score - options.minScore) / scoreRoom);
  const marginPart = Number.isFinite(margin)
    ? clamp01((margin - options.minMargin) / Math.max(options.minMargin, 0.05))
    : 1;
  const confidence = clamp01(0.5 + 0.25 * scorePart + 0.25 * marginPart);
  return { match: best, confidence, reason: "matched" };
}

export function clamp01(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(1, value));
}
