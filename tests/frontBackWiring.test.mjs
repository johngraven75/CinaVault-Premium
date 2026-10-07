import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

import { DUPLICATE_MODES, formatBytes, keeperId } from "../src/utils/duplicates.ts";

const read = (path) => fs.readFileSync(path, "utf8");

test("duplicate modes offered in the UI are exactly the ones find_duplicates accepts", () => {
  const rust = read("src-tauri/src/duplicates.rs");
  const accepted = rust.match(/\[("name_size"[^\]]*)\]\.contains/);
  assert.ok(accepted, "find_duplicates mode list not found in duplicates.rs");
  const rustModes = [...accepted[1].matchAll(/"([a-z_]+)"/g)].map((m) => m[1]).sort();
  assert.deepEqual(DUPLICATE_MODES.map((m) => m.id).sort(), rustModes);
});

test("keeperId picks the largest copy, first one on ties", () => {
  const group = (sizes) => ({
    key: "k",
    count: sizes.length,
    total_size: sizes.reduce((a, b) => a + b, 0),
    files: sizes.map((size, i) => ({ id: i + 1, path: `D:/m${i}.mkv`, name: `m${i}`, size })),
  });
  assert.equal(keeperId(group([700, 4_000, 1_500])), 2);
  assert.equal(keeperId(group([900, 900])), 1);
  assert.equal(keeperId(group([])), undefined);
});

test("formatBytes renders binary units", () => {
  assert.equal(formatBytes(0), "0 B");
  assert.equal(formatBytes(512), "512 B");
  assert.equal(formatBytes(1536), "1.5 KB");
  assert.equal(formatBytes(40 * 1024 ** 3), "40 GB");
});

test("Media Sources polls real scan progress and can cancel a scan", () => {
  const tab = read("src/components/tabs/MediaSourcesTab.tsx");
  assert.match(tab, /invoke<[^>]*>\("get_scan_progress"\)/);
  assert.match(tab, /setScanProgress\(/);
  assert.match(tab, /invoke\("cancel_scan"\)/);
  const scanner = read("src-tauri/src/scanner.rs");
  assert.match(scanner, /"total": SCAN_TOTAL/);
  assert.match(scanner, /"current": SCAN_CURRENT/);
});

test("default player picker stores executables play_media can launch", () => {
  const settings = read("src/components/tabs/SettingsTab.tsx");
  assert.match(settings, /invoke<PlayerInfo\[\]>\("get_available_players"\)/);
  assert.match(settings, /invoke\("set_default_player", \{ player \}\)/);
  assert.doesNotMatch(settings, /<option value="(vlc|mpv|vidstack)">/);
  const player = read("src-tauri/src/player.rs");
  assert.match(player, /player_exe == "system"/);
});

test("Advanced tab only enables switches that something reads", () => {
  const advanced = read("src/components/tabs/AdvancedTab.tsx");
  const wired = advanced.match(/WIRED_FEATURES = new Set<string>\(\[([^\]]*)\]\)/);
  assert.ok(wired, "WIRED_FEATURES not found");
  const keys = [...wired[1].matchAll(/"([a-z_]+)"/g)].map((m) => m[1]);
  assert.deepEqual(keys, ["particle_bg"]);
  assert.match(read("src/components/tabs/HomeTab.tsx"), /featureSettings\.particle_bg\?\.enabled !== false/);
  assert.match(advanced, /disabled=\{!WIRED_FEATURES\.has\(feature\.key\)\}/);
  assert.match(advanced, /invoke\("set_setting", \{ key: REQUESTS_SETTING_KEY/);
});

test("duplicate finder calls the registered commands with their argument names", () => {
  const finder = read("src/components/library/DuplicateFinder.tsx");
  assert.match(finder, /invoke<DuplicateScanResult>\("find_duplicates", \{ mode \}\)/);
  assert.match(finder, /invoke<string>\("quarantine", \{ itemId: file\.id \}\)/);
  const lib = read("src-tauri/src/lib.rs");
  assert.match(lib, /duplicates::find_duplicates/);
  assert.match(lib, /duplicates::quarantine/);
});

test("unused front-end packages stay removed", () => {
  const pkg = JSON.parse(read("package.json"));
  for (const name of ["clsx", "mdns-js", "react-virtuoso"]) {
    assert.equal(pkg.dependencies[name], undefined, `${name} should not be a dependency`);
  }
});

test("this app ships without the CinaVault Plus paywall", () => {
  for (const path of [
    "src-tauri/src/entitlements.rs",
    "src-tauri/src/metadata_paywall.rs",
    "src/services/entitlements.ts",
    "src/components/paywall",
    "src/components/tabs/AccountTab.tsx",
  ]) {
    assert.equal(fs.existsSync(path), false, `${path} should not exist`);
  }
  assert.doesNotMatch(read("src-tauri/src/lib.rs"), /entitlements::|metadata_paywall/);
  assert.doesNotMatch(read("src-tauri/src/embedded_server.rs"), /PAYMENT_REQUIRED/);
});

test("vision end-to-end check loads the same model the app ships", () => {
  const smoke = read("scripts/vision-smoke.mjs");
  const vision = read("src/services/localVision.ts");
  const fetcher = read("scripts/fetch-ai-models.mjs");
  const value = (source, name) => source.match(new RegExp(`${name}[^=]*= "([^"]+)"`))?.[1];
  assert.equal(value(smoke, "MODEL_ID"), value(vision, "VISION_MODEL_ID"));
  assert.equal(value(smoke, "REVISION"), value(vision, "VISION_MODEL_REVISION"));
  assert.equal(value(smoke, "REVISION"), value(fetcher, "REVISION"));
  assert.match(vision, /VISION_MODEL_DTYPE = "q8"/);
  assert.match(smoke, /dtype: "q8"/);
  assert.match(smoke, /env\.allowRemoteModels = false;/);
  assert.match(read(".github/workflows/windows-installer.yml"), /run: npm run fetch:ai-models\n\n      - name: Offline vision model end to end\n        run: npm run test:vision-e2e/);
});
