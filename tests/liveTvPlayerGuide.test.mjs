import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  buildAddM3uArgs,
  dayLabel,
  detectStreamKind,
  formatDuration,
  isPlayableUrl,
  programmeProgress,
  remainingLabel,
  splitGuide,
} from "../src/utils/iptvStream.ts";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(resolve(ROOT, path), "utf8").replace(/\r\n/g, "\n");

test("streams pick hls.js, mpegts.js or the native decoder", () => {
  assert.equal(detectStreamKind("http://cdn.example/live/master.m3u8?token=1"), "hls");
  assert.equal(detectStreamKind("http://provider.example:8080/live/u/p/42.ts"), "mpegts");
  assert.equal(detectStreamKind("http://provider.example:8080/u/p/42"), "mpegts");
  assert.equal(detectStreamKind("http://provider.example/play?id=4&output=m3u8"), "hls");
  assert.equal(detectStreamKind("https://files.example/clip.mp4"), "native");
  assert.equal(detectStreamKind("not a url.m3u8"), "hls");
});

test("only http and https streams are playable", () => {
  assert.ok(isPlayableUrl("http://a.example/x.ts"));
  assert.ok(isPlayableUrl(" https://a.example/x.m3u8 "));
  assert.equal(isPlayableUrl("file:///C:/x.ts"), false);
  assert.equal(isPlayableUrl("rtmp://a.example/live"), false);
  assert.equal(isPlayableUrl("a.example/x.ts"), false);
});

test("guide splits into now and next and measures progress", () => {
  const now = 1_000_000;
  const guide = [
    { start: now + 1800, stop: now + 3600, title: "Later" },
    { start: now - 600, stop: now + 1800, title: "Now" },
    { start: now - 900, stop: now + 300, title: "Overlap from another guide" },
    { start: now + 3600, stop: now + 5400, title: "Last" },
  ];
  const { current, upcoming } = splitGuide(guide, now);
  assert.equal(current.title, "Now", "the most recently started overlap wins");
  assert.deepEqual(upcoming.map((item) => item.title), ["Later", "Last"]);
  assert.equal(programmeProgress({ start: now - 600, stop: now + 600 }, now), 0.5);
  assert.equal(programmeProgress({ start: now + 10, stop: now + 600 }, now), 0);
  assert.equal(programmeProgress({ start: now - 600, stop: now - 10 }, now), 1);
  assert.equal(programmeProgress({ start: now, stop: now }, now), 0);
  assert.equal(splitGuide([], now).current, null);
});

test("durations and labels read naturally", () => {
  assert.equal(formatDuration(45 * 60), "45 min");
  assert.equal(formatDuration(90 * 60), "1 h 30 min");
  assert.equal(formatDuration(120 * 60), "2 h");
  assert.equal(remainingLabel({ stop: 1000 }, 970), "Ending now");
  assert.equal(remainingLabel({ stop: 1000 + 12 * 60 }, 1000), "12 min left");
  const noon = new Date(2026, 9, 11, 12, 0, 0).getTime() / 1000;
  assert.equal(dayLabel(noon + 3600, noon), null);
  assert.equal(dayLabel(noon + 86_400, noon), "Tomorrow");
  assert.ok(dayLabel(noon + 2 * 86_400, noon));
});

test("M3U playlist form requires a full URL", () => {
  assert.deepEqual(buildAddM3uArgs({ name: " ", url: " http://p.example/get.php?type=m3u_plus " }), {
    name: "M3U playlist",
    url: "http://p.example/get.php?type=m3u_plus",
  });
  assert.throws(() => buildAddM3uArgs({ name: "x", url: "p.example/list.m3u" }));
});

test("Live TV tab, player and Rust commands are wired together", () => {
  const lib = read("src-tauri/src/lib.rs");
  const tab = read("src/components/tabs/LiveTVTab.tsx");
  const player = read("src/components/IPTVPlayer.tsx");
  for (const command of [
    "add_m3u_playlist",
    "get_stream_relay_url",
    "list_epg_sources",
    "add_epg_source",
    "remove_epg_source",
    "sync_epg_source",
    "sync_stale_epg_sources",
    "get_channel_guide",
    "get_now_playing",
  ]) {
    assert.ok(lib.includes(`::${command},`), `lib.rs registers ${command}`);
    assert.ok(tab.includes(`"${command}"`) || player.includes(`"${command}"`), `UI calls ${command}`);
  }
  assert.ok(player.includes('import("hls.js")'), "hls.js loads on demand");
  assert.ok(player.includes('import("mpegts.js")'), "mpegts.js loads on demand");
  assert.ok(player.includes("enableWorker: false"), "players stay inside the CSP (no blob workers)");
});

test("CSP lets the player attach MSE streams and reach the relay", () => {
  const conf = JSON.parse(read("src-tauri/tauri.conf.json"));
  const csp = conf.app.security.csp;
  const media = csp.split(";").find((part) => part.trim().startsWith("media-src"));
  assert.ok(media.includes("blob:"), "media-src allows blob: for MediaSource");
  assert.ok(media.includes("http://127.0.0.1:*"), "media-src allows the loopback relay");
});
