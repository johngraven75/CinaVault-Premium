import test from "node:test";
import assert from "node:assert/strict";

import {
  isAutopilotEnabled,
  runPosterPass,
  selectPassCandidates,
} from "../src/services/aiPosterAutopilot.ts";

const item = (id, extra = {}) => ({
  id,
  title: `Film ${id}`,
  file_path: `/m/${id}.mkv`,
  media_type: "movie",
  verified: false,
  watched: false,
  favorite: false,
  date_added: "2026-10-01",
  poster_path: `/m/${id}.jpg`,
  ...extra,
});

test("selectPassCandidates keeps unverified titled items with posters, adult last, capped", () => {
  const items = [
    item(1, { media_type: "adult" }),
    item(2, { verified: true }),
    item(3, { poster_path: "" }),
    item(4, { title: "  " }),
    item(5),
    item(6),
    { ...item(7), id: undefined },
  ];
  assert.deepEqual(selectPassCandidates(items).map((entry) => entry.id), [5, 6, 1]);
  assert.deepEqual(selectPassCandidates(items, 2).map((entry) => entry.id), [5, 6]);
  assert.deepEqual(selectPassCandidates(items, 0), []);
});

test("isAutopilotEnabled is on unless explicitly turned off", () => {
  assert.equal(isAutopilotEnabled(null), true);
  assert.equal(isAutopilotEnabled(""), true);
  assert.equal(isAutopilotEnabled("true"), true);
  assert.equal(isAutopilotEnabled(" FALSE "), false);
});

test("runPosterPass verifies matches, flags wrong posters, and survives errors", async () => {
  const marked = [];
  const verdicts = {
    1: { ok: true, score: 0.9, posterLikelihood: 0.95 },
    2: { ok: false, score: 0.1, posterLikelihood: 0.9 },
    3: { ok: false, score: 0.1, posterLikelihood: 0.2 },
  };
  const result = await runPosterPass({
    listUnverified: async () => [item(1), item(2), item(3), item(4), item(5)],
    posterSource: async (entry) => (entry.id === 4 ? null : `src-${entry.id}`),
    verify: async (entry, src) => {
      assert.equal(src, `src-${entry.id}`);
      if (entry.id === 5) throw new Error("decode failed");
      return verdicts[entry.id];
    },
    markVerified: async (id) => {
      marked.push(id);
    },
  });
  assert.deepEqual(marked, [1]);
  assert.deepEqual(result, { checked: 3, verified: [1], flagged: [2], skipped: 1, errors: 1 });
});

test("runPosterPass stops when asked", async () => {
  let calls = 0;
  const result = await runPosterPass({
    listUnverified: async () => [item(1), item(2)],
    posterSource: async () => "src",
    verify: async () => {
      calls += 1;
      return { ok: true, score: 1, posterLikelihood: 1 };
    },
    markVerified: async () => {},
    shouldStop: () => calls >= 1,
  });
  assert.equal(calls, 1);
  assert.deepEqual(result.verified, [1]);
});
