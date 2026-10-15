// Build 171 — Siri / iOS App Intents link-up and accessibility.
//
// Logic tests import the pure routing module (via the TS loader) and exercise
// parse → match → plan → execute. Structural tests pin the native Swift layer,
// the Info.plist keys, the app wiring, and the accessibility additions so they
// cannot silently drift. Run with:
//   node --loader ./tests/typescript-loader.mjs --test tests/build171SiriIntents.test.mjs

import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

import {
  SIRI_URL_SCHEME,
  parseSiriIntent,
  normalizeTitle,
  matchLibraryItem,
  planSiriIntent,
} from "../src/services/siriIntents.ts";
import { executeSiriPlan, executeSiriUrl } from "../src/services/siriBridge.ts";

const read = (path) => fs.readFileSync(path, "utf8");

const library = [
  { title: "Blade Runner 2049", file_path: "/m/blade2049.mkv", media_type: "movie" },
  { title: "Dune", file_path: "/m/dune.mkv", media_type: "movie" },
  { title: "Dune: Part Two", file_path: "/m/dune2.mkv", media_type: "movie" },
];

// ── Parsing ──────────────────────────────────────────────────────────────

test("scheme constant is cinavault", () => {
  assert.equal(SIRI_URL_SCHEME, "cinavault");
});

test("parses each intent kind from the URL", () => {
  assert.deepEqual(parseSiriIntent("cinavault://play?title=Dune"), { kind: "play", query: "Dune" });
  assert.deepEqual(parseSiriIntent("cinavault://search?q=sci%20fi"), { kind: "search", query: "sci fi" });
  assert.deepEqual(parseSiriIntent("cinavault://ask?q=what%20is%20new"), { kind: "ask", query: "what is new" });
  assert.deepEqual(parseSiriIntent("cinavault://pause"), { kind: "pause" });
  assert.deepEqual(parseSiriIntent("cinavault://resume"), { kind: "resume" });
  assert.deepEqual(parseSiriIntent("cinavault://skip"), { kind: "skip" });
});

test("accepts title/q/query/text parameter aliases", () => {
  assert.equal(parseSiriIntent("cinavault://search?query=Dune")?.query, "Dune");
  assert.equal(parseSiriIntent("cinavault://search?text=Dune")?.query, "Dune");
});

test("rejects foreign schemes, unknown actions, and subject-less play/search/ask", () => {
  assert.equal(parseSiriIntent("https://example.com/play?title=Dune"), null);
  assert.equal(parseSiriIntent("cinavault://explode"), null);
  assert.equal(parseSiriIntent("cinavault://play"), null);
  assert.equal(parseSiriIntent("cinavault://ask"), null);
  assert.equal(parseSiriIntent("not a url"), null);
  assert.equal(parseSiriIntent(""), null);
});

// ── Matching ─────────────────────────────────────────────────────────────

test("normalizeTitle strips punctuation and case", () => {
  assert.equal(normalizeTitle("Dune: Part Two!"), "dune part two");
});

test("matchLibraryItem resolves exact, partial, and token overlap; null on no overlap", () => {
  assert.equal(matchLibraryItem(library, "dune")?.title, "Dune");
  assert.equal(matchLibraryItem(library, "blade runner")?.title, "Blade Runner 2049");
  assert.equal(matchLibraryItem(library, "dune part two")?.title, "Dune: Part Two");
  assert.equal(matchLibraryItem(library, "nonexistent flick"), null);
});

// ── Planning ─────────────────────────────────────────────────────────────

test("play with a confident match plans play; with none it degrades to search", () => {
  assert.deepEqual(planSiriIntent({ kind: "play", query: "dune" }, library), {
    action: "play",
    item: library[1],
  });
  assert.deepEqual(planSiriIntent({ kind: "play", query: "totally unknown" }, library), {
    action: "search",
    query: "totally unknown",
    reason: "no-match",
  });
});

test("transport and ask plans pass through", () => {
  assert.deepEqual(planSiriIntent({ kind: "pause" }, library), { action: "pause" });
  assert.deepEqual(planSiriIntent({ kind: "ask", query: "hi" }, library), { action: "ask", question: "hi" });
});

// ── Execution (routing through injected deps) ──────────────────────────────

function spyDeps(lib = library) {
  const calls = [];
  return {
    calls,
    getLibrary: () => lib,
    showLibrary: () => calls.push(["showLibrary"]),
    setSearch: (q) => calls.push(["setSearch", q]),
    playItem: (item) => calls.push(["playItem", item.title]),
    control: (c) => {
      calls.push(["control", c]);
      return true;
    },
    ask: (q) => calls.push(["ask", q]),
  };
}

test("play URL navigates to the library and plays the matched item", async () => {
  const deps = spyDeps();
  const result = await executeSiriUrl("cinavault://play?title=blade%20runner", deps);
  assert.equal(result.ok, true);
  assert.deepEqual(deps.calls, [["showLibrary"], ["playItem", "Blade Runner 2049"]]);
});

test("search URL navigates and sets the search box", async () => {
  const deps = spyDeps();
  await executeSiriUrl("cinavault://search?q=heist", deps);
  assert.deepEqual(deps.calls, [["showLibrary"], ["setSearch", "heist"]]);
});

test("unmatched play falls back to a search for the spoken title", async () => {
  const deps = spyDeps();
  await executeSiriPlan({ action: "search", query: "unknown", reason: "no-match" }, deps);
  assert.deepEqual(deps.calls, [["showLibrary"], ["setSearch", "unknown"]]);
});

test("transport URLs call the player controller", async () => {
  const deps = spyDeps();
  await executeSiriUrl("cinavault://pause", deps);
  await executeSiriUrl("cinavault://skip", deps);
  assert.deepEqual(deps.calls, [["control", "pause"], ["control", "skip"]]);
});

test("ask URL routes the question to the agent", async () => {
  const deps = spyDeps();
  await executeSiriUrl("cinavault://ask?q=what%20is%20new", deps);
  assert.deepEqual(deps.calls, [["ask", "what is new"]]);
});

test("an unrecognised URL is reported, not executed", async () => {
  const deps = spyDeps();
  const result = await executeSiriUrl("cinavault://explode", deps);
  assert.equal(result.ok, false);
  assert.equal(result.error, "unrecognised");
  assert.deepEqual(deps.calls, []);
});

// ── App wiring (web half) ───────────────────────────────────────────────────

test("App installs the Siri bridge with real store/player/agent actions", () => {
  const app = read("src/App.tsx");
  assert.match(app, /installSiriBridge\(/);
  assert.match(app, /controlPlayback\(command\)/);
  assert.match(app, /emitSiriAsk\(question\)/);
  assert.match(app, /playMedia\(/);
});

test("playback.ts exposes a transport controller for Siri", () => {
  const playback = read("src/services/playback.ts");
  assert.match(playback, /registerPlayerController/);
  assert.match(playback, /export async function controlPlayback/);
});

test("LibraryPlayer registers the transport controller", () => {
  const player = read("src/components/player/LibraryPlayer.tsx");
  assert.match(player, /registerPlayerController\(\(command\)/);
});

test("HoloAgent answers Siri questions via the ask event", () => {
  const agent = read("src/components/agent/HoloAgent.tsx");
  assert.match(agent, /SIRI_ASK_EVENT/);
  assert.match(agent, /askRef\.current\(question\)/);
});

// ── Native iOS half ─────────────────────────────────────────────────────────

const base = "platforms/ios/siri";

test("App Intents use the modern framework, not legacy SiriKit", () => {
  const intents = read(`${base}/CinaVaultIntents.swift`);
  assert.match(intents, /import AppIntents/);
  assert.doesNotMatch(intents, /import Intents\b/);
  // No legacy SiriKit base classes in use (a prose mention in a comment is fine).
  assert.doesNotMatch(intents, /:\s*INIntent\b/);
  assert.doesNotMatch(intents, /INExtension/);
  for (const name of ["PlayItemIntent", "SearchLibraryIntent", "PauseIntent", "ResumeIntent", "SkipIntent", "AskHoloIntent"]) {
    assert.match(intents, new RegExp(`struct ${name}: AppIntent`), `${name} missing`);
  }
  assert.match(intents, /openAppWhenRun: Bool = true/);
});

test("AppShortcutsProvider registers all six intents with applicationName phrases", () => {
  const shortcuts = read(`${base}/CinaVaultShortcuts.swift`);
  assert.match(shortcuts, /AppShortcutsProvider/);
  const providerIntents = [...shortcuts.matchAll(/intent:\s*([A-Za-z]+)\(\)/g)].map((m) => m[1]);
  assert.deepEqual(
    providerIntents.sort(),
    ["AskHoloIntent", "PauseIntent", "PlayItemIntent", "ResumeIntent", "SearchLibraryIntent", "SkipIntent"],
  );
  assert.match(shortcuts, /\\\(\.applicationName\)/);
});

test("SiriLink targets the cinavault scheme over a shared app group", () => {
  const link = read(`${base}/SiriLink.swift`);
  assert.match(link, /static let scheme = "cinavault"/);
  assert.match(link, /group\.com\.cinavault\.premium/);
  assert.match(link, /func takePending\(\)/);
});

test("Info.plist additions declare Siri usage and the URL scheme", () => {
  const plist = read(`${base}/Info.plist.additions.xml`);
  assert.match(plist, /NSSiriUsageDescription/);
  assert.match(plist, /<string>cinavault<\/string>/);
  assert.match(plist, /CFBundleURLTypes/);
});

test("Swift intent tests cover every intent", () => {
  const swiftTests = read(`${base}/Tests/CinaVaultIntentsTests.swift`);
  for (const t of ["testPlayIntent", "testSearchIntent", "testAskIntent", "testTransportIntents"]) {
    assert.match(swiftTests, new RegExp(t));
  }
});

// ── Accessibility ─────────────────────────────────────────────────────────

test("Sidebar exposes a labelled nav landmark and labels every item", () => {
  const sidebar = read("src/components/Sidebar.tsx");
  assert.match(sidebar, /<nav aria-label="Primary navigation"/);
  assert.match(sidebar, /aria-label=\{item\.label\}/);
});
