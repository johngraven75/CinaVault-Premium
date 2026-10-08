// Home screen discovery: pure shaping helpers (src/services/discoveryLogic.ts)
// and the wiring that keeps every discovery shelf behind its own switch.
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  carouselStep,
  continueWatchingItems,
  genreRadioQueue,
  isOnWatchlist,
  itemMediaIds,
  newReleasesMessage,
  previewText,
  progressFraction,
  shelfLimit,
  shuffled,
  stationSize,
  toShelfItems,
  wheelScrollDelta,
} from "../src/services/discoveryLogic.ts";

const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");

const media = (id, title, extra = {}) => ({
  id,
  title,
  file_path: `C:/m/${title}.mkv`,
  media_type: "movie",
  verified: false,
  watched: false,
  favorite: false,
  date_added: "2026-01-01T00:00:00Z",
  ...extra,
});

/** Deterministic [0,1) sequence. */
function sequence(values) {
  let i = 0;
  return () => values[i++ % values.length];
}

test("discovery entries become cards with their copies and reason", () => {
  const [card] = toShelfItems([
    {
      work_key: "tmdb:603",
      primary: media(7, "The Matrix"),
      copies: [
        { id: 7, file_path: "C:/m/The Matrix.mkv", source_id: 1, file_size: 10, resolution: "1080p" },
        { id: 9, file_path: "D:/m/matrix-4k.mkv", source_id: 2, file_size: 20, resolution: "2160p" },
      ],
      copy_count: 2,
      source_ids: [1, 2],
      reason: "Because you watched Heat",
      score: 0.8,
    },
  ]);
  assert.equal(card.item.title, "The Matrix");
  assert.equal(card.item.copy_count, 2);
  assert.deepEqual(card.item.copies.map((copy) => copy.id), [7, 9]);
  assert.equal(card.reason, "Because you watched Heat");
  assert.deepEqual(itemMediaIds(card.item), [7, 9]);
});

test("continue watching cards carry progress, resume point and time left", () => {
  const [card, unknown] = continueWatchingItems([
    { item: media(1, "Alpha"), progress: { media_id: 1, position: 1800, duration: 7200, finished: false, updated_at: "" } },
    { item: media(2, "Beta"), progress: { media_id: 2, position: 95, duration: 0, finished: false, updated_at: "" } },
  ]);
  assert.equal(card.progress, 0.25);
  assert.equal(card.resumeAt, 1800);
  assert.equal(card.reason, "1:30:00 left");
  assert.equal(unknown.progress, 0);
  assert.equal(unknown.reason, "Stopped at 1:35");
  assert.equal(progressFraction(9000, 7200), 1);
  assert.equal(progressFraction(Number.NaN, 7200), 0);
});

test("shuffle is a permutation driven by the random source", () => {
  const items = [1, 2, 3, 4, 5];
  const out = shuffled(items, sequence([0, 0, 0, 0]));
  assert.deepEqual(out, [2, 3, 4, 5, 1]);
  assert.deepEqual(items, [1, 2, 3, 4, 5], "input untouched");
  assert.deepEqual([...shuffled(items)].sort(), items);
  // A random source that returns 1 must not index past the end.
  assert.deepEqual([...shuffled(items, () => 1)].sort(), items);
});

test("genre radio queues only playable titles, shuffled", () => {
  const entry = (item) => ({ work_key: `k${item.id}`, primary: item, copies: [], copy_count: 1, source_ids: [] });
  const queue = genreRadioQueue(
    [entry(media(1, "A")), entry(media(2, "B", { file_path: "" })), entry(media(3, "C"))],
    (item) => Boolean(item.file_path),
    sequence([0]),
  );
  assert.deepEqual(queue.map((item) => item.title), ["C", "A"]);
});

test("watchlist membership checks every copy of a work", () => {
  const work = media(5, "Heat", { copies: [{ id: 5, file_path: "a" }, { id: 6, file_path: "b" }] });
  assert.equal(isOnWatchlist(work, new Set([6])), true);
  assert.equal(isOnWatchlist(work, new Set([1, 2])), false);
  assert.deepEqual(itemMediaIds(media(undefined, "No id")), []);
});

test("new releases message, preview text and limits", () => {
  assert.equal(newReleasesMessage(0), "Nothing new since your last visit");
  assert.equal(newReleasesMessage(1), "1 new title since your last visit");
  assert.equal(newReleasesMessage(1234), "1,234 new titles since your last visit");
  assert.equal(previewText("  A   short\n plot. "), "A short plot.");
  assert.equal(previewText("one two three four five six", 15), "one two three…");
  assert.equal(previewText(undefined), "");
  assert.equal(shelfLimit("12", 20), 12);
  assert.equal(shelfLimit(500, 20), 60);
  assert.equal(shelfLimit("nope", 20), 20);
  assert.equal(stationSize(2), 5);
  assert.equal(stationSize(undefined), 200);
});

test("carousel steps by whole cards and wheel scroll yields at the ends", () => {
  assert.equal(carouselStep(1000, 150, 14), 820); // 5 cards of 164px
  assert.equal(carouselStep(100, 150, 14), 164); // never less than one card
  assert.equal(wheelScrollDelta(0, 120, 0, 500), 120);
  assert.equal(wheelScrollDelta(0, -120, 0, 500), 0, "at the start, scrolling up moves the page");
  assert.equal(wheelScrollDelta(0, 120, 500, 500), 0, "at the end, scrolling down moves the page");
  assert.equal(wheelScrollDelta(-40, 5, 200, 500), -40, "horizontal trackpad swipes win");
});

test("each discovery shelf is gated by its own switch", () => {
  const shelves = read("src/components/discovery/DiscoveryShelves.tsx");
  for (const key of ["continue_watching", "watchlist", "new_releases"]) {
    assert.match(shelves, new RegExp(`isFeatureOn\\(featureSettings, "${key}"\\)`));
  }
  assert.match(shelves, /useFeature<TrendingConfig>\("trending"/);
  assert.match(shelves, /useFeature<RecommendationsConfig>\("recommendations"/);
  assert.match(shelves, /"cinavault:library-refresh"/);
  assert.match(read("src/components/discovery/MoreLikeThis.tsx"), /isFeatureOn\(state\.featureSettings, "similar_titles"\)/);
  assert.match(read("src/components/discovery/GenreRadio.tsx"), /useFeature<GenreRadioConfig>\("genre_radio"/);
  assert.match(read("src/components/discovery/GenreRadio.tsx"), /source: "genre_radio", shuffle: true/);
  assert.match(read("src/components/discovery/ShelfRow.tsx"), /isFeatureOn\(state\.featureSettings, "shelf_carousel"\)/);
  assert.match(read("src/components/discovery/PosterPreview.tsx"), /isFeatureOn\(state\.featureSettings, "poster_hover"\)/);
  assert.match(read("src/components/discovery/PosterPreview.tsx"), /data-poster-preview/);
});

test("both home skins render the discovery UI and Kodi reloads on library refresh", () => {
  const home = read("src/components/tabs/HomeTab.tsx");
  const kodi = read("src/components/kodi/KodiHomeLayout.tsx");
  for (const source of [home, kodi]) {
    assert.match(source, /<DiscoveryShelves skin="(holo|kodi)"/);
    assert.match(source, /<MoreLikeThis item=/);
    assert.match(source, /<WatchlistButton item=/);
    assert.match(source, /<PosterPreview item=/);
  }
  assert.match(kodi, /<ShelfRow label=\{title\}/);
  assert.match(kodi, /addEventListener\("cinavault:library-refresh", loadLibrary\)/);
  assert.match(read("src/main.tsx"), /import "\.\/styles\/discovery\.css";/);
});

test("discovery commands are registered and gated in the back end", () => {
  const lib = read("src-tauri/src/lib.rs");
  const rust = read("src-tauri/src/discovery.rs");
  for (const command of [
    "discovery_recommendations",
    "discovery_similar",
    "discovery_trending",
    "discovery_new_releases",
    "discovery_genres",
    "discovery_genre_queue",
  ]) {
    assert.match(lib, new RegExp(`discovery::${command},`));
    assert.match(rust, new RegExp(`pub (async )?fn ${command}\\(`));
  }
  for (const key of ["recommendations", "similar_titles", "trending", "new_releases", "genre_radio"]) {
    assert.match(rust, new RegExp(`feature_flags::is_enabled\\(db, "${key}"\\)`));
  }
  assert.match(rust, /TRENDING_TTL_HOURS: i64 = 6/);
  assert.match(rust, /new_releases_seen_/);
});
