import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

import {
  GENRES,
  KIND_PROMPTS,
  POSTER_READ_MODEL,
  TAG_NEUTRAL_PROMPT,
  TAG_PROMPTS,
  analyzePoster,
  describePoster,
  genreField,
  insightsFromScores,
  parsePosterRead,
  readPosterText,
  yearToFill,
} from "../src/services/posterVision.ts";
import { buildRecord, isSettingOn, runVisionPass, summarizeVisionPass } from "../src/services/posterVisionPass.ts";
import { seeImage } from "../src/services/agentVision.ts";

const read = (path) => readFile(fileURLToPath(new URL(path, import.meta.url)), "utf8").then((text) => text.replace(/\r/g, ""));

const scores = (entries) => Object.entries(entries).map(([label, score]) => ({ label, score }));

const horror = {
  kind: scores({ movie: 0.7, series: 0.1, screenshot: 0.05, photo: 0.15 }),
  genre: scores({ Horror: 0.5, Thriller: 0.3, Comedy: 0.05, Drama: 0.15 }),
  mood: scores({ dark: 0.6, bright: 0.1, tense: 0.3 }),
  era: scores({ "1980s": 0.55, modern: 0.45 }),
  tags: scores({ monsters: 0.9, blood: 0.75, cars: 0.4, couple: 0.69 }),
};

test("insightsFromScores keeps strong genres, moods, era and tags", () => {
  const insights = insightsFromScores(horror);
  assert.equal(insights.isPoster, true);
  assert.equal(insights.kind, "movie");
  assert.deepEqual(insights.genres, ["Horror", "Thriller"]);
  assert.deepEqual(insights.moods, ["dark"]);
  assert.equal(insights.era, "1980s");
  assert.deepEqual(insights.tags, ["monsters", "blood"]);
  assert.equal(insights.confidence, 0.5);
  assert.equal(genreField(insights), "Horror, Thriller");
});

test("insightsFromScores drops genres and era when the image is not a poster", () => {
  const insights = insightsFromScores({ ...horror, kind: scores({ movie: 0.2, screenshot: 0.7 }) });
  assert.equal(insights.isPoster, false);
  assert.equal(insights.kind, "screenshot");
  assert.deepEqual(insights.genres, []);
  assert.equal(insights.era, null);
  assert.equal(genreField(insights), null);
});

test("weak, flat genre scores produce no genre guess", () => {
  const flat = Object.fromEntries(GENRES.map((genre) => [genre, 1 / GENRES.length]));
  const insights = insightsFromScores({ ...horror, genre: scores(flat) });
  assert.deepEqual(insights.genres, []);
  assert.equal(genreField(insights), null);
});

test("analyzePoster maps prompts back to keys and tests each tag against the neutral prompt", async () => {
  const calls = [];
  const classify = async (src, labels) => {
    calls.push(labels);
    assert.equal(src, "poster.jpg");
    if (labels.includes(TAG_NEUTRAL_PROMPT)) {
      const hit = labels[0] === TAG_PROMPTS.spaceships ? 0.95 : 0.2;
      return [
        { label: labels[0], score: hit },
        { label: TAG_NEUTRAL_PROMPT, score: 1 - hit },
      ];
    }
    if (labels.includes(KIND_PROMPTS.movie)) return labels.map((label) => ({ label, score: label === KIND_PROMPTS.movie ? 0.8 : 0.2 / 7 }));
    if (labels.some((label) => label.includes("science fiction")))
      return labels.map((label) => ({ label, score: label.includes("science fiction") ? 0.6 : 0.4 / 17 }));
    return labels.map((label, index) => ({ label, score: index === 0 ? 0.9 : 0.1 / (labels.length - 1) }));
  };
  const insights = await analyzePoster("poster.jpg", classify);
  assert.equal(insights.kind, "movie");
  assert.deepEqual(insights.genres, ["Science Fiction"]);
  assert.deepEqual(insights.tags, ["spaceships"]);
  assert.equal(calls.length, 4 + Object.keys(TAG_PROMPTS).length);
});

test("parsePosterRead accepts fenced JSON and rejects junk and impossible years", () => {
  const now = new Date("2026-10-12");
  assert.deepEqual(
    parsePosterRead('```json\n{"title":"  The  Thing ","year":"1982","tagline":"Man is the warmest place to hide","caption":"A man in a parka."}\n```', now),
    { title: "The Thing", year: 1982, tagline: "Man is the warmest place to hide", caption: "A man in a parka." },
  );
  assert.equal(parsePosterRead('{"title":null,"year":3000,"caption":"x"}', now).year, null);
  assert.equal(parsePosterRead('{"title":"null"}', now).title, null);
  assert.equal(parsePosterRead("no json here", now), null);
  assert.equal(parsePosterRead("{broken", now), null);
  assert.equal(parsePosterRead(42, now), null);
});

test("readPosterText uses the open Qwen3-VL model and survives failures", async () => {
  let seen;
  const ok = await readPosterText("data:image/png;base64,AA", async (args) => {
    seen = args;
    return { status: "success", message: '{"title":"Alien","year":1979,"caption":"An egg."}' };
  });
  assert.equal(seen.model, POSTER_READ_MODEL);
  assert.equal(seen.imageUrl, "data:image/png;base64,AA");
  assert.equal(ok.title, "Alien");
  assert.equal(await readPosterText("x", async () => ({ status: "error", message: "401" })), null);
  assert.equal(await readPosterText("x", async () => { throw new Error("offline"); }), null);
});

test("yearToFill only trusts a printed year when the printed title matches", () => {
  const read = { title: "Alien", year: 1979, tagline: null, caption: null };
  assert.equal(yearToFill("Alien", null, read), 1979);
  assert.equal(yearToFill("Alien", 1979, read), null);
  assert.equal(yearToFill("Predator", null, read), null);
  assert.equal(yearToFill("Alien", null, null), null);
});

test("describePoster reads like a sentence", () => {
  const insights = insightsFromScores(horror);
  const text = describePoster(insights, { title: "The Thing", year: 1982, tagline: null, caption: "A man in the snow" });
  assert.match(text, /^It reads "The Thing" \(1982\)\. A man in the snow\. /);
  assert.match(text, /a movie poster for horror and thriller, dark in tone, styled like the 1980s\./);
  assert.match(text, /I can see monsters and blood\./);
});

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

test("buildRecord never asks to replace an existing genre", () => {
  const insights = insightsFromScores(horror);
  assert.equal(buildRecord(item(1), insights, null).fillGenre, "Horror, Thriller");
  assert.equal(buildRecord(item(1, { genre: "Drama" }), insights, null).fillGenre, null);
  assert.deepEqual(buildRecord(item(1), insights, null).models, ["Xenova/clip-vit-base-patch32"]);
  const read = { title: "Film 1", year: 1999, tagline: null, caption: "c" };
  const record = buildRecord(item(1), insights, read);
  assert.equal(record.fillYear, 1999);
  assert.equal(record.caption, "c");
  assert.equal(record.models.length, 2);
});

test("runVisionPass saves records, caps cloud reads, keeps adult art local and survives errors", async () => {
  const saved = [];
  const readCalls = [];
  const result = await runVisionPass(
    {
      listPending: async (limit) => {
        assert.equal(limit, 10);
        return [item(1), item(2, { media_type: "adult" }), item(3), item(4), item(5), { ...item(6), id: undefined }];
      },
      posterSource: async (entry) => (entry.id === 4 ? null : `src-${entry.id}`),
      analyze: async (src) => {
        if (src === "src-5") throw new Error("decode failed");
        return insightsFromScores(horror);
      },
      read: async (src) => {
        readCalls.push(src);
        return { title: null, year: null, tagline: null, caption: "x" };
      },
      save: async (id, record) => {
        saved.push([id, record.caption]);
        return { genreFilled: id === 1, yearFilled: false };
      },
      cloudReadLimit: 1,
    },
    10,
  );
  assert.deepEqual(readCalls, ["src-1"]);
  assert.deepEqual(saved, [[1, "x"], [2, null], [3, null]]);
  assert.deepEqual(result, { analyzed: 3, genresFilled: 1, yearsFilled: 0, cloudReads: 1, skipped: 2, errors: 1 });
  assert.equal(summarizeVisionPass(result), "AI vision read 3 posters and filled in 1 genre.");
  assert.equal(summarizeVisionPass({ ...result, analyzed: 0 }), null);
});

test("isSettingOn defaults on", () => {
  assert.equal(isSettingOn(null), true);
  assert.equal(isSettingOn(" False "), false);
});

test("seeImage answers offline with local vision and uses Qwen3-VL when a token is set", async () => {
  const analyze = async () => insightsFromScores(horror);
  const offline = await seeImage("data:x", "what is this?", {
    analyze,
    hasToken: async () => false,
    infer: async () => assert.fail("no cloud call without a token"),
    read: async () => assert.fail("no cloud call without a token"),
  });
  assert.match(offline, /a movie poster for horror and thriller/);
  assert.match(offline, /Hugging Face token/);

  const online = await seeImage("data:x", "what is this?", {
    analyze,
    hasToken: async () => true,
    infer: async (args) => {
      assert.equal(args.model, POSTER_READ_MODEL);
      return { status: "success", message: "It's the poster for The Thing." };
    },
    read: async () => assert.fail("questions go to infer"),
  });
  assert.match(online, /^It's the poster for The Thing\.\n\nTo me it looks like/);

  const failed = await seeImage("data:x", "", {
    analyze: async () => { throw new Error("model missing"); },
    hasToken: async () => false,
    infer: async () => null,
    read: async () => null,
  });
  assert.match(failed, /couldn't look at that image/);
});

test("the scan, boot and agent paths are wired to the vision skill", async () => {
  const [app, sources, agent, lib] = await Promise.all([
    read("../src/App.tsx"),
    read("../src/components/tabs/MediaSourcesTab.tsx"),
    read("../src/components/agent/HoloAgent.tsx"),
    read("../src-tauri/src/lib.rs"),
  ]);
  assert.match(app, /startVisionPass\(\)/);
  assert.match(sources, /followPostScan\(\)[\s\S]*startVisionPass\(\)/);
  assert.match(agent, /seeImage\(image\.preview, clean\)/);
  assert.doesNotMatch(agent, /useClaude \? attachment : null/);
  for (const command of ["media_vision_pending", "media_vision_save", "media_vision_get", "media_vision_forget"]) {
    assert.match(lib, new RegExp(`media_vision::${command}`));
  }
});
