import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

import {
  cosineSimilarity,
  decideMatch,
  meanEmbedding,
  normalize,
  rankCandidates,
  softmax,
} from "../src/services/visionMath.ts";
import {
  AUTO_APPLY_THRESHOLDS,
  decideIdentification,
  decidePosterVerification,
  identifyFromCandidates,
  imageSimilarityToScore,
  normalizeSearchResults,
  normalizeTitle,
  posterPrompt,
  posterVerificationLabels,
  scoreCandidates,
  titleSimilarity,
  yearSimilarity,
} from "../src/services/posterIdentification.ts";
import { LruCache, VISION_MODEL_ID, VISION_MODEL_REVISION } from "../src/services/localVision.ts";

const close = (actual, expected, epsilon = 1e-6) =>
  assert.ok(Math.abs(actual - expected) < epsilon, `${actual} !~ ${expected}`);

// ── visionMath ───────────────────────────────────────────────────────────────

test("cosineSimilarity handles identical, orthogonal, opposite, zero and mismatched vectors", () => {
  close(cosineSimilarity([1, 2, 3], [2, 4, 6]), 1);
  close(cosineSimilarity([1, 0], [0, 1]), 0);
  close(cosineSimilarity([1, 0], [-1, 0]), -1);
  assert.equal(cosineSimilarity([0, 0], [1, 1]), 0);
  assert.equal(cosineSimilarity([1, 2], [1, 2, 3]), 0);
  assert.equal(cosineSimilarity([], []), 0);
});

test("normalize returns unit vectors and leaves zero vectors at zero", () => {
  const unit = normalize([3, 4]);
  close(unit[0], 0.6);
  close(unit[1], 0.8);
  assert.deepEqual(Array.from(normalize([0, 0, 0])), [0, 0, 0]);
  const mean = meanEmbedding([[1, 0], [0, 1]]);
  close(Math.hypot(mean[0], mean[1]), 1);
});

test("softmax is stable for large logits and sums to one", () => {
  const probabilities = softmax([0.31, 0.29, 0.2], 100);
  close(probabilities.reduce((sum, value) => sum + value, 0), 1);
  assert.ok(probabilities[0] > probabilities[1] && probabilities[1] > probabilities[2]);
  assert.ok(softmax([1000, 1000]).every((value) => Math.abs(value - 0.5) < 1e-9));
  assert.deepEqual(softmax([]), []);
});

test("rankCandidates sorts by similarity and keeps input order on ties", () => {
  const ranked = rankCandidates([1, 0], [
    { id: "far", embedding: [0, 1] },
    { id: "near", embedding: [0.9, 0.1] },
    { id: "tieA", embedding: [0.5, 0.5] },
    { id: "tieB", embedding: [0.5, 0.5] },
  ]);
  assert.deepEqual(ranked.map((entry) => entry.id), ["near", "tieA", "tieB", "far"]);
  assert.ok(ranked[0].score > 0.99);
});

test("decideMatch requires both minimum score and margin", () => {
  const opts = { minScore: 0.8, minMargin: 0.05 };
  assert.deepEqual(decideMatch([], opts), { match: null, confidence: 0, reason: "no_candidates" });
  assert.equal(decideMatch([{ id: "a", score: 0.7 }], opts).reason, "below_min_score");
  assert.equal(decideMatch([{ id: "a", score: 0.9 }, { id: "b", score: 0.88 }], opts).reason, "ambiguous");
  const single = decideMatch([{ id: "a", score: 0.9 }], opts);
  assert.equal(single.reason, "matched");
  assert.equal(single.match.id, "a");
  assert.ok(single.confidence > 0.5 && single.confidence <= 1);
  const strong = decideMatch([{ id: "a", score: 0.99 }, { id: "b", score: 0.2 }], opts);
  const weak = decideMatch([{ id: "a", score: 0.81 }, { id: "b", score: 0.75 }], opts);
  assert.ok(strong.confidence > weak.confidence);
  assert.equal(decideMatch([{ id: "a", score: Number.NaN }], opts).reason, "below_min_score");
});

// ── posterIdentification: titles ─────────────────────────────────────────────

test("normalizeTitle strips release tags, years, punctuation and leading articles", () => {
  assert.equal(normalizeTitle("The.Matrix.1999.1080p.BluRay.x264"), "matrix");
  assert.equal(normalizeTitle("Amélie (2001) [4K]"), "amelie");
  assert.equal(normalizeTitle("Fast & Furious"), "fast and furious");
  assert.equal(normalizeTitle("Matrix, The (1999)"), "matrix");
  assert.equal(normalizeTitle("Beautiful Mind, A"), "beautiful mind");
  assert.equal(normalizeTitle("Them, the Others"), "them the others");
});

test("titleSimilarity is 1 for equivalent titles and low for unrelated ones", () => {
  assert.equal(titleSimilarity("The Matrix", "Matrix, The (1999)"), 1);
  assert.ok(titleSimilarity("The Matrix Reloaded", "The Matrix") > 0.5);
  assert.ok(titleSimilarity("The Matrix", "Finding Nemo") < 0.3);
  assert.equal(titleSimilarity("", "Anything"), 0);
});

test("yearSimilarity tolerates off-by-one and ignores missing years", () => {
  assert.equal(yearSimilarity(1999, 1999), 1);
  assert.equal(yearSimilarity(1999, 2000), 0.7);
  assert.equal(yearSimilarity(1999, 2005), 0);
  assert.equal(yearSimilarity(undefined, 1999), null);
});

test("imageSimilarityToScore maps CLIP cosine onto 0..1", () => {
  assert.equal(imageSimilarityToScore(0.3), 0);
  assert.equal(imageSimilarityToScore(0.99), 1);
  close(imageSimilarityToScore(0.725), 0.5);
});

// ── posterIdentification: search_metadata normalization ─────────────────────

test("normalizeSearchResults understands TMDb, OMDb, TVmaze, ThePornDB and Nuxt shapes", () => {
  const tmdb = normalizeSearchResults("tmdb", {
    results: [
      { id: 603, media_type: "movie", title: "The Matrix", release_date: "1999-03-30", poster_path: "/abc.jpg" },
      { id: 1, media_type: "person", name: "Keanu Reeves" },
      { id: 1399, media_type: "tv", name: "Game of Thrones", first_air_date: "2011-04-17", poster_path: null },
    ],
  });
  assert.equal(tmdb.length, 2);
  assert.deepEqual(
    { id: tmdb[0].id, year: tmdb[0].year, poster: tmdb[0].posterUrl, tmdbId: tmdb[0].tmdbId },
    { id: "tmdb:603", year: 1999, poster: "https://image.tmdb.org/t/p/w342/abc.jpg", tmdbId: "603" },
  );
  assert.equal(tmdb[1].title, "Game of Thrones");
  assert.equal(tmdb[1].posterUrl, null);

  const omdb = normalizeSearchResults("omdb", {
    Search: [{ Title: "Alien", Year: "1979", imdbID: "tt0078748", Type: "movie", Poster: "N/A" }],
  });
  assert.deepEqual(
    { id: omdb[0].id, year: omdb[0].year, imdb: omdb[0].imdbId, poster: omdb[0].posterUrl },
    { id: "omdb:tt0078748", year: 1979, imdb: "tt0078748", poster: null },
  );

  const tvmaze = normalizeSearchResults("tvmaze", [
    { score: 0.9, show: { id: 82, name: "Game of Thrones", premiered: "2011-04-17", image: { medium: "https://m/x.jpg", original: "https://o/x.jpg" }, externals: { imdb: "tt0944947" } } },
  ]);
  assert.equal(tvmaze[0].posterUrl, "https://o/x.jpg");
  assert.equal(tvmaze[0].imdbId, "tt0944947");

  const tpdb = normalizeSearchResults("tpdb", {
    data: [{ id: "abc", title: "Scene One", date: "2020-01-02", images: [{ url: "https://cdn/p.jpg" }] }],
  });
  assert.deepEqual([tpdb[0].id, tpdb[0].year, tpdb[0].posterUrl], ["tpdb:abc", 2020, "https://cdn/p.jpg"]);

  const nuxt = normalizeSearchResults("porn_site_nuxt", { status: "success", results: [{ title: "Untagged" }] });
  assert.equal(nuxt[0].id, "porn_site_nuxt:#0");

  assert.deepEqual(normalizeSearchResults("x", { message: "Provider integration pending." }), []);
  assert.deepEqual(normalizeSearchResults("x", null), []);
});

// ── posterIdentification: scoring and decisions ──────────────────────────────

const matrixItem = { title: "The.Matrix.1999.1080p", media_type: "movie", year: 1999 };
const candidates = normalizeSearchResults("tmdb", {
  results: [
    { id: 604, media_type: "movie", title: "The Matrix Reloaded", release_date: "2003-05-15", poster_path: "/r.jpg" },
    { id: 603, media_type: "movie", title: "The Matrix", release_date: "1999-03-30", poster_path: "/m.jpg" },
    { id: 605, media_type: "movie", title: "The Matrix Revolutions", release_date: "2003-11-05", poster_path: "/v.jpg" },
  ],
});

test("scoreCandidates ranks the exact title + year first and auto-applies it", () => {
  const ranked = scoreCandidates(matrixItem, candidates);
  assert.equal(ranked[0].candidate.id, "tmdb:603");
  assert.equal(ranked[0].titleScore, 1);
  assert.equal(ranked[0].yearScore, 1);
  assert.equal(ranked[0].imageScore, null);
  const decision = decideIdentification(ranked);
  assert.equal(decision.autoApply, true);
  assert.equal(decision.best.candidate.id, "tmdb:603");
  assert.ok(decision.confidence >= 0.5);
});

test("image similarity can break a title tie, and ambiguity blocks auto-apply", () => {
  const twins = normalizeSearchResults("tmdb", {
    results: [
      { id: 1, media_type: "movie", title: "Dune", release_date: "1984-12-14", poster_path: "/1984.jpg" },
      { id: 2, media_type: "movie", title: "Dune", release_date: "2021-09-15", poster_path: "/2021.jpg" },
    ],
  });
  const item = { title: "Dune", media_type: "movie" };
  const noImage = decideIdentification(scoreCandidates(item, twins));
  assert.equal(noImage.autoApply, false);
  assert.equal(noImage.reason, "ambiguous");
  assert.equal(noImage.best, null);

  const withImage = decideIdentification(
    scoreCandidates(item, twins, new Map([["tmdb:1", 0.55], ["tmdb:2", 0.96]])),
  );
  assert.equal(withImage.best.candidate.id, "tmdb:2");
  assert.equal(withImage.autoApply, true);
});

test("a weak-but-clear best candidate is suggested without auto-apply", () => {
  const ranked = scoreCandidates({ title: "Matrx", media_type: "movie" }, normalizeSearchResults("tmdb", {
    results: [{ id: 9, media_type: "movie", title: "The Matrix", release_date: "1999-01-01" }],
  }));
  const decision = decideIdentification(ranked);
  assert.ok(ranked[0].score < AUTO_APPLY_THRESHOLDS.minScore);
  assert.equal(decision.autoApply, false);
  assert.equal(decision.best?.candidate.id, "tmdb:9");
  assert.equal(decision.reason, "below_min_score");
});

test("identifyFromCandidates accepts the raw search_metadata response without a reference image", async () => {
  const result = await identifyFromCandidates(
    matrixItem,
    { results: [{ id: 603, media_type: "movie", title: "The Matrix", release_date: "1999-03-30" }] },
    { provider: "tmdb" },
  );
  assert.equal(result.best.candidate.id, "tmdb:603");
  assert.equal(result.autoApply, true);
  const empty = await identifyFromCandidates(matrixItem, [], {});
  assert.equal(empty.reason, "no_candidates");
});

test("poster verification prompts and decision", () => {
  assert.equal(posterPrompt({ title: "Alien", media_type: "movie", year: 1979 }), 'a movie poster for "Alien" (1979)');
  assert.match(posterPrompt({ title: "Lost", media_type: "tv" }), /^a TV show poster/);
  const labels = posterVerificationLabels({ title: "Alien", media_type: "movie" });
  assert.equal(labels.length, 7);

  const good = decidePosterVerification(labels, new Map([[labels[0], 0.8], [labels[1], 0.1], [labels[2], 0.1]]));
  assert.equal(good.ok, true);
  close(good.score, 0.8);
  close(good.posterLikelihood, 0.9);

  const otherFilm = decidePosterVerification(labels, new Map([[labels[0], 0.4], [labels[1], 0.6]]));
  assert.equal(otherFilm.ok, false);

  const notPoster = decidePosterVerification(labels, new Map([[labels[0], 0.05], [labels[3], 0.95]]));
  assert.equal(notPoster.ok, false);
  assert.ok(notPoster.posterLikelihood < 0.1);
});

// ── localVision pure pieces ──────────────────────────────────────────────────

test("LruCache evicts least recently used entries", () => {
  const cache = new LruCache(2);
  cache.set("a", 1);
  cache.set("b", 2);
  cache.get("a");
  cache.set("c", 3);
  assert.equal(cache.has("b"), false);
  assert.equal(cache.get("a"), 1);
  assert.equal(cache.get("c"), 3);
  assert.equal(cache.size, 2);
});

test("model id and fetch script agree, and the vision service lazy-loads transformers", async () => {
  const script = await readFile(new URL("../scripts/fetch-ai-models.mjs", import.meta.url), "utf8");
  assert.ok(script.includes(`"${VISION_MODEL_ID}"`));
  assert.ok(script.includes("onnx/vision_model_quantized.onnx"));
  assert.ok(script.includes("onnx/text_model_quantized.onnx"));
  assert.ok(script.includes(`"${VISION_MODEL_REVISION}"`), "fetch script and runtime must pin the same revision");
  assert.match(VISION_MODEL_REVISION, /^[0-9a-f]{40}$/, "revision must be an immutable commit id");
  const source = await readFile(new URL("../src/services/localVision.ts", import.meta.url), "utf8");
  assert.ok(!/^import .* from "@huggingface\/transformers"/m.test(source), "transformers must be imported lazily");
  assert.ok(source.includes('import("@huggingface/transformers")'));
});

test("model fetch accepts only files that match the pinned digests", async () => {
  const { MODEL_FILES, digestOf, matchesManifest } = await import("../scripts/fetch-ai-models.mjs");
  const { mkdtemp, writeFile, rm } = await import("node:fs/promises");
  const { tmpdir } = await import("node:os");
  const { join } = await import("node:path");
  const { execFileSync } = await import("node:child_process");

  for (const [file, spec] of Object.entries(MODEL_FILES)) {
    assert.ok(Number.isInteger(spec.size) && spec.size > 0, `${file} needs a size`);
    if (file.endsWith(".onnx")) assert.match(spec.sha256 ?? "", /^[0-9a-f]{64}$/, `${file} needs a SHA-256`);
    else assert.match(spec.gitSha1 ?? "", /^[0-9a-f]{40}$/, `${file} needs a git blob id`);
  }

  const dir = await mkdtemp(join(tmpdir(), "cv-model-"));
  try {
    const path = join(dir, "config.json");
    const content = '{"model_type":"clip"}\n';
    await writeFile(path, content);
    const size = Buffer.byteLength(content);
    // Same id git itself computes for the blob.
    const gitSha1 = execFileSync("git", ["hash-object", path], { encoding: "utf8" }).trim();
    assert.equal(await digestOf(path, { size, gitSha1 }), gitSha1);
    assert.equal(await matchesManifest(path, { size, gitSha1 }), true);

    const { createHash } = await import("node:crypto");
    const sha256 = createHash("sha256").update(content).digest("hex");
    assert.equal(await matchesManifest(path, { size, sha256 }), true);

    // Same size, different bytes: rejected.
    await writeFile(path, content.replace("clip", "evil"));
    assert.equal(await matchesManifest(path, { size, gitSha1 }), false);
    assert.equal(await matchesManifest(path, { size, sha256 }), false);
    assert.equal(await matchesManifest(join(dir, "missing.json"), { size, sha256 }), false);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
