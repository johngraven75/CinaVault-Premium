import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  DEFAULT_PLAYER_SETTINGS,
  PLAYER_SETTINGS_KEY,
  hlsConfigFor,
  mpegtsConfigFor,
  normalizePlayerSettings,
  pickHlsLevel,
  readPlayerSettings,
  savePlayerSettings,
} from "../src/utils/iptvPlayerSettings.ts";
import { EMPTY_SOURCE_FORM, LOGIN_TYPES, buildIptvSourceArgs, loginTypeInfo } from "../src/utils/iptvSource.ts";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(resolve(ROOT, path), "utf8").replace(/\r\n/g, "\n");

function memoryStorage(initial = {}) {
  const data = new Map(Object.entries(initial));
  return { getItem: (key) => data.get(key) ?? null, setItem: (key, value) => data.set(key, value), data };
}

test("player settings fall back to defaults and clamp stored values", () => {
  assert.deepEqual(readPlayerSettings(memoryStorage()), DEFAULT_PLAYER_SETTINGS);
  assert.deepEqual(readPlayerSettings(memoryStorage({ [PLAYER_SETTINGS_KEY]: "{not json" })), DEFAULT_PLAYER_SETTINGS);
  const settings = normalizePlayerSettings({ latency: "warp", bufferSeconds: 999, reconnectAttempts: -4, quality: "lowest", videoFit: "cover", autoplay: false });
  assert.deepEqual(settings, { ...DEFAULT_PLAYER_SETTINGS, bufferSeconds: 120, reconnectAttempts: 0, quality: "lowest", videoFit: "cover", autoplay: false });

  const storage = memoryStorage();
  savePlayerSettings({ ...DEFAULT_PLAYER_SETTINGS, latency: "stable" }, storage);
  assert.equal(readPlayerSettings(storage).latency, "stable");
});

test("latency modes map to hls.js and mpegts.js options", () => {
  const low = hlsConfigFor({ ...DEFAULT_PLAYER_SETTINGS, latency: "low", bufferSeconds: 20 });
  assert.equal(low.lowLatencyMode, true);
  assert.equal(low.maxBufferLength, 20);
  assert.equal(low.enableWorker, false);
  const stable = hlsConfigFor({ ...DEFAULT_PLAYER_SETTINGS, latency: "stable" });
  assert.equal(stable.lowLatencyMode, false);
  assert.ok(stable.liveSyncDurationCount > low.liveSyncDurationCount);

  assert.equal(mpegtsConfigFor({ ...DEFAULT_PLAYER_SETTINGS, latency: "low" }).liveBufferLatencyChasing, true);
  assert.equal(mpegtsConfigFor({ ...DEFAULT_PLAYER_SETTINGS, latency: "stable" }).liveBufferLatencyChasing, false);
  assert.equal(mpegtsConfigFor(DEFAULT_PLAYER_SETTINGS).enableWorker, false);
});

test("quality preference picks an hls level", () => {
  assert.equal(pickHlsLevel("auto", 4), -1);
  assert.equal(pickHlsLevel("highest", 4), 3);
  assert.equal(pickHlsLevel("lowest", 4), 0);
  assert.equal(pickHlsLevel("highest", 1), -1);
});

test("each login type sends only the fields it uses", () => {
  assert.deepEqual(LOGIN_TYPES.map((type) => type.id), ["xtream", "m3u", "stalker"]);
  assert.equal(loginTypeInfo("unknown").id, "xtream");

  const xtream = buildIptvSourceArgs({
    ...EMPTY_SOURCE_FORM,
    name: " TV ",
    serverUrl: "provider.example:8080/player_api.php",
    username: " viewer ",
    secret: " pw ",
    userAgent: " VLC/3.0 ",
  });
  assert.deepEqual(xtream, {
    source: { kind: "xtream", name: "TV", serverUrl: "http://provider.example:8080", username: "viewer", secret: "pw", userAgent: "VLC/3.0", referrer: null },
  });
  assert.throws(() => buildIptvSourceArgs({ ...EMPTY_SOURCE_FORM, serverUrl: "p.example", username: "u" }), /Password is required/);
  assert.equal(
    buildIptvSourceArgs({ ...EMPTY_SOURCE_FORM, serverUrl: "p.example", username: "u" }, { editing: true }).source.secret,
    null,
    "editing keeps the saved password",
  );

  const m3u = buildIptvSourceArgs({ ...EMPTY_SOURCE_FORM, kind: "m3u", serverUrl: "https://list.example/a.m3u", username: "x", secret: "y", referrer: "https://site.example/" });
  assert.equal(m3u.source.username, null);
  assert.equal(m3u.source.secret, null);
  assert.equal(m3u.source.referrer, "https://site.example/");
  assert.throws(() => buildIptvSourceArgs({ ...EMPTY_SOURCE_FORM, kind: "m3u", serverUrl: "list.example/a.m3u" }));

  const stalker = buildIptvSourceArgs({ ...EMPTY_SOURCE_FORM, kind: "stalker", serverUrl: "http://portal.example/c/", secret: "00:1a:79:ab:cd:ef" });
  assert.equal(stalker.source.secret, "00:1a:79:ab:cd:ef");
  assert.throws(() => buildIptvSourceArgs({ ...EMPTY_SOURCE_FORM, kind: "stalker", serverUrl: "http://portal.example/c/", secret: "00:1A" }), /MAC/);
  assert.throws(() => buildIptvSourceArgs({ ...EMPTY_SOURCE_FORM, kind: "m3u", serverUrl: "https://a.example/x", userAgent: "a\nb" }), /line breaks/);
});

test("the player has a settings button and passes headers to the relay", () => {
  const player = read("src/components/IPTVPlayer.tsx");
  const tab = read("src/components/tabs/LiveTVTab.tsx");
  assert.match(player, /aria-label="Player settings"/);
  assert.match(player, /invoke<string>\("get_stream_relay_url", \{ url, userAgent, referrer \}\)/);
  assert.match(tab, /invoke<Playback>\("resolve_live_stream", \{ channelId: selectedChannel\.id \}\)/);
  assert.match(tab, /invoke\("update_iptv_source", \{ id, \.\.\.args \}\)/);
  assert.match(tab, /invoke<number>\("add_iptv_source", args\)/);
});
