import test from "node:test";
import assert from "node:assert/strict";

import {
  activeSkip,
  bufferHold,
  bufferPlan,
  bufferedAhead,
  chapterKind,
  chooseRoute,
  directPlayMime,
  fadeSeconds,
  formatClock,
  formatVttTime,
  nextCaptionChoice,
  parseVttTime,
  shiftVtt,
  nextUpRemaining,
  nextUpStart,
  resumeDecision,
  routeAfterError,
  shouldPreloadNext,
  shouldSaveProgress,
  skipWindows,
  transcodeSrc,
} from "../src/services/playerLogic.ts";

const probe = (over = {}) => ({
  duration: 2700,
  formatName: "mov,mp4,m4a,3gp,3g2,mj2",
  videoCodec: "h264",
  videoProfile: "High",
  pixFmt: "yuv420p",
  audioCodec: "aac",
  chapters: [],
  ...over,
});

// A WebView2-like decoder: MP4/WebM with H.264/VP9/AV1 + AAC/Opus, no Matroska, no HEVC.
const webview = (mime) =>
  /^video\/(mp4|webm)/.test(mime) && !/hvc1|ac-3|ec-3/.test(mime) ? "probably" : "";

test("direct play MIME names container and codecs", () => {
  assert.equal(directPlayMime("C:\\M\\a.mp4", probe()), 'video/mp4; codecs="avc1.640028, mp4a.40.2"');
  assert.equal(directPlayMime("a.webm", probe({ videoCodec: "vp9", audioCodec: "opus" })), 'video/webm; codecs="vp09.00.40.08, opus"');
  assert.equal(directPlayMime("a.MOV", null), "video/mp4");
  assert.equal(directPlayMime("a.avi", probe()), null);
  assert.equal(directPlayMime("a.mp4", probe({ videoCodec: "mpeg4" })), null);
  assert.equal(directPlayMime("a.mp4", probe({ audioCodec: "dts" })), null);
  assert.equal(directPlayMime("a.mp4", probe({ videoProfile: "High 10", pixFmt: "yuv420p10le" })), null);
});

test("route: direct when decodable, transcode otherwise, external when forced or no ffmpeg", () => {
  const base = { canPlayType: webview, forceDirectPlay: false, transcodeAvailable: true };
  assert.deepEqual(chooseRoute({ ...base, filePath: "a.mp4", probe: probe() }), { kind: "direct" });
  assert.deepEqual(chooseRoute({ ...base, filePath: "a.mkv", probe: probe() }), { kind: "transcode" });
  assert.deepEqual(chooseRoute({ ...base, filePath: "a.mp4", probe: probe({ audioCodec: "ac3" }) }), { kind: "transcode" });

  const forced = chooseRoute({ ...base, forceDirectPlay: true, filePath: "a.avi", probe: probe({ videoCodec: "mpeg4", audioCodec: "mp3" }) });
  assert.equal(forced.kind, "external");
  assert.match(forced.reason, /Force Direct Play is on/);
  assert.match(forced.reason, /AVI \/ MPEG4 \/ MP3/);
  // Forced but decodable still plays in the app.
  assert.deepEqual(chooseRoute({ ...base, forceDirectPlay: true, filePath: "a.mp4", probe: probe() }), { kind: "direct" });

  const noFfmpeg = chooseRoute({ ...base, transcodeAvailable: false, filePath: "a.mkv", probe: probe() });
  assert.equal(noFfmpeg.kind, "external");
  assert.match(noFfmpeg.reason, /ffmpeg/);
});

test("after a playback error: direct falls to transcode, transcode falls to external", () => {
  const input = { filePath: "a.mp4", probe: probe(), forceDirectPlay: false, transcodeAvailable: true };
  assert.deepEqual(routeAfterError({ kind: "direct" }, input), { kind: "transcode" });
  assert.equal(routeAfterError({ kind: "direct" }, { ...input, forceDirectPlay: true }).kind, "external");
  assert.equal(routeAfterError({ kind: "transcode" }, input).kind, "external");
});

test("transcode URLs carry the start offset", () => {
  assert.equal(transcodeSrc("http://127.0.0.1:5000/t/transcode/4", 0), "http://127.0.0.1:5000/t/transcode/4");
  assert.equal(transcodeSrc("http://127.0.0.1:5000/t/transcode/4", 61.25), "http://127.0.0.1:5000/t/transcode/4?start=61.250");
  assert.equal(transcodeSrc("u", Number.NaN), "u");
});

test("chapter names map to intro and credits", () => {
  for (const name of ["Intro", "Opening", "OP", "OP 2", "Opening Credits", "Cold Open / Intro"]) assert.equal(chapterKind(name), "intro", name);
  for (const name of ["Credits", "End Credits", "Ending", "ED", "Outro"]) assert.equal(chapterKind(name), "credits", name);
  for (const name of ["Chapter 3", "Operation", "Edward", ""]) assert.equal(chapterKind(name), null, name);
});

const skipOn = { introEnabled: true, creditsEnabled: true, introSeconds: 90, creditsSeconds: 120 };

test("skip windows come from chapters first", () => {
  const chapters = [
    { start: 0, end: 30, title: "Recap" },
    { start: 30, end: 120, title: "Opening" },
    { start: 120, end: 1300, title: "Part A" },
    { start: 1300, end: 1420, title: "Ending" },
  ];
  const w = skipWindows(chapters, 1420, skipOn);
  assert.deepEqual(w.intro, { start: 30, end: 120, source: "chapter" });
  assert.deepEqual(w.credits, { start: 1300, end: 1420, source: "chapter" });
  assert.equal(activeSkip(10, w), null);
  assert.equal(activeSkip(30, w), "intro");
  assert.equal(activeSkip(119.5, w), null);
  assert.equal(activeSkip(1350, w), "credits");
});

test("skip windows fall back to configured seconds, only for long files and enabled switches", () => {
  const w = skipWindows([], 3600, skipOn);
  assert.deepEqual(w.intro, { start: 0, end: 90, source: "window" });
  assert.deepEqual(w.credits, { start: 3480, end: 3600, source: "window" });
  assert.deepEqual(skipWindows([], 300, skipOn), { intro: null, credits: null });
  assert.deepEqual(skipWindows([], 3600, { ...skipOn, introEnabled: false, creditsSeconds: 0 }), { intro: null, credits: null });
  // An intro chapter in the second half is not an intro.
  assert.equal(skipWindows([{ start: 2000, end: 2100, title: "Intro" }], 3600, { ...skipOn, introSeconds: 0 }).intro, null);
});

test("next up appears at credits or N seconds before the end and counts down in playback time", () => {
  const settings = { countdownSeconds: 10, secondsBeforeEnd: 30 };
  assert.equal(nextUpStart(1420, { start: 1300, end: 1420, source: "chapter" }, settings), 1300);
  assert.equal(nextUpStart(1420, null, settings), 1390);
  assert.equal(nextUpStart(60, null, settings), 45);
  assert.equal(nextUpStart(0, null, settings), null);
  assert.equal(nextUpRemaining(1300, 1300, 10), 10);
  assert.equal(nextUpRemaining(1304.2, 1300, 10), 6);
  assert.equal(nextUpRemaining(1400, 1300, 10), 0);
});

test("gapless preloads near the end", () => {
  assert.equal(shouldPreloadNext(100, 3600, null), false);
  assert.equal(shouldPreloadNext(3520, 3600, null), true);
  assert.equal(shouldPreloadNext(3280, 3600, 3300), true);
  assert.equal(shouldPreloadNext(10, 0, null), false);
});

test("resume rules", () => {
  const saved = { position: 754, duration: 2700, finished: false };
  assert.deepEqual(resumeDecision(saved, true), { position: 754, reason: "resume" });
  assert.deepEqual(resumeDecision(saved, false), { position: 0, reason: "start" });
  assert.deepEqual(resumeDecision({ ...saved, finished: true }, true), { position: 0, reason: "start" });
  assert.deepEqual(resumeDecision({ ...saved, position: 14.9 }, true), { position: 0, reason: "start" });
  assert.deepEqual(resumeDecision({ ...saved, position: 2697 }, true), { position: 0, reason: "start" });
  assert.deepEqual(resumeDecision(null, true), { position: 0, reason: "start" });
  assert.deepEqual(resumeDecision(saved, true, 42), { position: 42, reason: "requested" });
  assert.equal(shouldSaveProgress(0, 9_999, 0, 10), false);
  assert.equal(shouldSaveProgress(0, 10_000, 0, 10), true);
  assert.equal(shouldSaveProgress(0, 20_000, 10, 10.5), false);
});

test("buffer plan mirrors the server and holds stalls until the target", () => {
  assert.deepEqual(bufferPlan(false, { bufferAheadSeconds: 60 }), { preload: "metadata", bufferAheadSeconds: 0, chunkKb: 64 });
  assert.deepEqual(bufferPlan(true, {}), { preload: "auto", bufferAheadSeconds: 30, chunkKb: 256 });
  assert.deepEqual(bufferPlan(true, { bufferAheadSeconds: "90", chunkKb: 1 }), { preload: "auto", bufferAheadSeconds: 90, chunkKb: 16 });
  assert.equal(bufferedAhead([[0, 10], [20, 50]], 25), 25);
  assert.equal(bufferedAhead([[0, 10]], 15), 0);
  assert.equal(bufferHold(5, 30, 1000, 0), "hold");
  assert.equal(bufferHold(30, 30, 1000, 0), "resume");
  assert.equal(bufferHold(8, 30, 9, 0), "resume");
  assert.equal(bufferHold(1, 30, 1000, 15_000), "resume");
  assert.equal(bufferHold(0, 0, 1000, 0), "resume");
  assert.equal(bufferHold(5, 30, 1000, 0, false), "resume");
});

test("crossfade length and clock formatting", () => {
  assert.equal(fadeSeconds(false, { seconds: 3 }), 0);
  assert.equal(fadeSeconds(true, {}), 1.5);
  assert.equal(fadeSeconds(true, { seconds: 30 }), 10);
  assert.equal(formatClock(754.9), "12:34");
  assert.equal(formatClock(3725), "1:02:05");
  assert.equal(formatClock(Number.NaN), "0:00");
});

test("vtt times parse and format", () => {
  assert.equal(parseVttTime("01:02:03.500"), 3723.5);
  assert.equal(parseVttTime("02:03.250"), 123.25);
  assert.ok(Number.isNaN(parseVttTime("bad")));
  assert.equal(formatVttTime(3723.5), "01:02:03.500");
  assert.equal(formatVttTime(-4), "00:00:00.000");
});

test("captions shift to a transcode's seek point", () => {
  const vtt = "WEBVTT\n\n1\n00:00:05.000 --> 00:00:08.000\nGone\n\n2\n00:00:09.000 --> 00:00:12.000 align:start\nSplit\n\n3\n00:01:00.000 --> 00:01:02.000\nLater";
  assert.equal(shiftVtt(vtt, 0), vtt);
  assert.equal(
    shiftVtt(vtt, 10),
    "WEBVTT\n\n2\n00:00:00.000 --> 00:00:02.000 align:start\nSplit\n\n3\n00:00:50.000 --> 00:00:52.000\nLater",
  );
});

test("the CC button cycles off, each track, then off", () => {
  assert.equal(nextCaptionChoice(-1, 0), -1);
  assert.equal(nextCaptionChoice(-1, 2), 0);
  assert.equal(nextCaptionChoice(0, 2), 1);
  assert.equal(nextCaptionChoice(1, 2), -1);
});
