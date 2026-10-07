import test from "node:test";
import assert from "node:assert/strict";

import {
  copyCountLabel,
  copyFileName,
  formatFileSize,
  hasMultipleCopies,
  loadUnifiedLibrary,
  mapUnifiedEntries,
  mediaItemsAsSingleCopies,
  unifiedEntryToMediaItem,
} from "../src/services/unifiedLibrary.ts";

function item(overrides = {}) {
  return {
    id: 1,
    title: "Heat",
    file_path: "D:/Movies/Heat (1995) 2160p.mkv",
    media_type: "movie",
    year: 1995,
    resolution: "2160p",
    file_size: 40_000_000_000,
    source_id: 1,
    verified: true,
    watched: false,
    favorite: false,
    date_added: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

test("an entry maps to one card: primary fields + copy_count + copies (primary first)", () => {
  const card = unifiedEntryToMediaItem({
    work_key: "tmdb:949",
    primary: item({ id: 7, file_path: "//nas/movies/Heat.2160p.mkv", poster_path: "C:/p.jpg" }),
    copies: [
      { id: 3, file_path: "D:/Movies/Heat 720p.mp4", source_id: 2, file_size: 1_500_000_000, resolution: "720p" },
      { id: 7, file_path: "//nas/movies/Heat.2160p.mkv", source_id: 1, file_size: 40_000_000_000, resolution: "2160p" },
    ],
    copy_count: 2,
    source_ids: [1, 2],
  });
  assert.equal(card.id, 7);
  assert.equal(card.title, "Heat");
  assert.equal(card.poster_path, "C:/p.jpg");
  assert.equal(card.work_key, "tmdb:949");
  assert.equal(card.copy_count, 2);
  assert.deepEqual(card.copies.map((copy) => copy.id), [7, 3]);
  assert.equal(hasMultipleCopies(card), true);
  assert.equal(copyCountLabel(card), "×2 copies");
});

test("missing or malformed copies fall back to the primary file", () => {
  const card = unifiedEntryToMediaItem({
    work_key: "",
    primary: item({ id: 9 }),
    copies: [null, { id: 4, file_path: "" }],
    copy_count: 0,
    source_ids: [],
  });
  assert.equal(card.copy_count, 1);
  assert.equal(card.copies.length, 1);
  assert.equal(card.copies[0].file_path, "D:/Movies/Heat (1995) 2160p.mkv");
  assert.equal(card.copies[0].source_id, 1);
  assert.equal(card.work_key, "id:9");
  assert.equal(hasMultipleCopies(card), false);
});

test("copy_count never under-reports the copies listed", () => {
  const card = unifiedEntryToMediaItem({
    work_key: "title:heat|1995|movie",
    primary: item({ id: 1 }),
    copies: [
      { id: 1, file_path: "a.mkv", source_id: 1, file_size: 1, resolution: null },
      { id: 2, file_path: "b.mkv", source_id: 1, file_size: 1, resolution: null },
      { id: 3, file_path: "c.mkv", source_id: 1, file_size: 1, resolution: null },
    ],
    copy_count: 2,
    source_ids: [1],
  });
  assert.equal(card.copy_count, 3);
});

test("mapUnifiedEntries drops entries without a primary file", () => {
  const cards = mapUnifiedEntries([
    { work_key: "a", primary: item({ id: 1 }), copies: [], copy_count: 1, source_ids: [1] },
    { work_key: "b", primary: null, copies: [], copy_count: 1, source_ids: [] },
  ]);
  assert.equal(cards.length, 1);
  assert.deepEqual(mapUnifiedEntries(undefined), []);
});

test("fallback items become single-copy cards", async () => {
  const [card] = mediaItemsAsSingleCopies([item({ id: 5 })]);
  assert.equal(card.copy_count, 1);
  assert.equal(card.copies[0].id, 5);

  // Outside Tauri the unified command rejects, so the fallback loader is used.
  const result = await loadUnifiedLibrary("movie", async () => [item({ id: 11 })]);
  assert.equal(result.unified, false);
  assert.equal(result.items[0].id, 11);
  assert.equal(result.items[0].copy_count, 1);
  assert.ok(result.error);
});

test("copy formatting helpers", () => {
  assert.equal(formatFileSize(null), "Unknown size");
  assert.equal(formatFileSize(512), "512 B");
  assert.equal(formatFileSize(1_500_000_000), "1.4 GB");
  assert.equal(copyFileName({ file_path: "C:\\Media\\Heat.mkv" }), "Heat.mkv");
  assert.equal(copyFileName({ file_path: "//nas/share/Heat.mkv" }), "Heat.mkv");
});
