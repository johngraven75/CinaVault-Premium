// Library & Metadata switches: pure front-end helpers, and the wiring
// between the panels, the services and the Tauri commands they call.
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  boundedInt,
  chapterIntervalMinutes,
  describePostScanRun,
  describePosterSyncRun,
  describeSubtitleRun,
  formatLanguageList,
  groupCollections,
  parseLanguageList,
} from "../src/features/library/libraryAutomation.ts";

const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");

test("subtitle languages are normalized like subtitles.rs does", () => {
  assert.deepEqual(parseLanguageList("EN, pt_BR;fr  en bad!code"), ["en", "pt-br", "fr"]);
  assert.deepEqual(parseLanguageList(""), []);
  assert.equal(formatLanguageList(["en", "fr"]), "en, fr");
  assert.equal(formatLanguageList(undefined), "");
});

test("numeric settings are clamped to what the back end accepts", () => {
  assert.equal(chapterIntervalMinutes(undefined), 5);
  assert.equal(chapterIntervalMinutes("2"), 2);
  assert.equal(chapterIntervalMinutes(0.1), 0.5);
  assert.equal(chapterIntervalMinutes(600), 60);
  assert.equal(chapterIntervalMinutes(1.3), 1.5);
  assert.equal(boundedInt("12", 8, 1, 30), 12);
  assert.equal(boundedInt("x", 8, 1, 30), 8);
  assert.equal(boundedInt(99, 2, 2, 50), 50);
});

test("run summaries report real counts and the no-key skip", () => {
  assert.equal(describeSubtitleRun(null), "No automatic run yet.");
  assert.match(describeSubtitleRun({ status: "skipped_no_key" }), /^Skipped: no OpenSubtitles API key/);
  assert.equal(
    describeSubtitleRun({ status: "partial", items: 3, downloaded: 2, alreadyPresent: 1, notFound: 0, errors: ["x"] }),
    "2 downloaded, 1 already present, 0 not found for 3 title(s), 1 error(s)",
  );
  assert.equal(
    describePosterSyncRun({ trigger: "manual", exported: 4, unchanged: 10, imported: 1, errors: [] }),
    "Sync now: 4 copied to the folder, 10 unchanged, 1 imported",
  );
  assert.deepEqual(
    describePostScanRun({
      items: 3,
      metadata: { items: 3, updated: 2 },
      chapterThumbs: { generated: 1, skipped: 2 },
      collections: { series: 1, franchises: 2, genres: 3 },
    }),
    [
      "3 new title(s)",
      "Metadata: 2 of 3 matched",
      "Chapter thumbnails: 1 made, 2 skipped",
      "Collections: 2 franchise, 1 series, 3 genre",
    ],
  );
});

test("collections are grouped franchise, series, genre and largest first", () => {
  const c = (id, kind, name, itemCount) => ({ id, key: `${kind}:${name}`, name, kind, itemCount, posterPath: null, updatedAt: "" });
  const groups = groupCollections([c(1, "genre", "Drama", 9), c(2, "series", "Severance", 19), c(3, "franchise", "Alien", 4), c(4, "genre", "Action", 12)]);
  assert.deepEqual(
    groups.map((g) => [g.label, g.collections.map((x) => x.id)]),
    [["Franchises", [3]], ["Series", [2]], ["Genres", [4, 1]]],
  );
});

test("panels and services call commands the back end registers", () => {
  const lib = read("src-tauri/src/lib.rs");
  const calls = [
    ["src/features/panels/subtitle_fetch.tsx", ["subtitles_status", "subtitles_find", "set_api_key", "search_media"]],
    ["src/features/panels/poster_sync.tsx", ["poster_sync_status", "poster_sync_now"]],
    ["src/features/panels/auto_metadata.tsx", ["post_scan_status"]],
    ["src/services/collections.ts", ["collections_list", "collection_items", "collections_rebuild"]],
    ["src/services/postScan.ts", ["post_scan_status"]],
  ];
  for (const [file, commands] of calls) {
    const source = read(file);
    for (const command of commands) {
      assert.match(source, new RegExp(`invoke(<[^>]*>)?\\("${command}"`), `${file} invokes ${command}`);
      assert.match(lib, new RegExp(`::${command},`), `${command} is registered in lib.rs`);
    }
  }
});

test("scans hand new titles to the post-scan worker and record scan.finished", () => {
  const scanner = read("src-tauri/src/scanner.rs");
  assert.match(scanner, /crate::post_scan::schedule\(report\.added_ids\.clone\(\)\)/);
  assert.match(scanner, /"scan\.finished"/);
  assert.match(scanner, /feature_flags::is_enabled\(db, "smart_match"\)/);
  assert.match(scanner, /feature_flags::is_enabled\(db, "nfo_import"\)/);
  const worker = read("src-tauri/src/post_scan.rs");
  for (const key of ["auto_metadata", "subtitle_fetch", "chapter_thumbs", "collection_auto", "poster_sync"]) {
    assert.match(worker, new RegExp(`is_enabled\\(db, "${key}"\\)`), `post_scan reads ${key}`);
  }
  assert.match(worker, /"metadata\.updated"/);
  assert.match(read("src-tauri/src/subtitles.rs"), /"subtitles\.downloaded"/);
  const sources = read("src/components/tabs/MediaSourcesTab.tsx");
  assert.match(sources, /useFeature\("auto_metadata"\)/);
  assert.match(sources, /followPostScan\(\)/);
});
