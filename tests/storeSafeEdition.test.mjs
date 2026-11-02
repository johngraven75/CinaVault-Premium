import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

const read = (path) => fs.readFileSync(path, "utf8");

test("store-safe edition is a build mode of the single Premium codebase", () => {
  assert.match(read(".env.store-safe"), /VITE_STORE_SAFE=true/);
  const edition = read("src/config/edition.ts");
  assert.match(edition, /IS_STORE_SAFE = import\.meta\.env\.VITE_STORE_SAFE === "true"/);
  for (const fn of ["filterStoreSafeMedia", "filterStoreSafeSources", "filterStoreSafeProviders"]) {
    assert.match(edition, new RegExp(`export function ${fn}`));
    assert.match(read("src/store/appStore.ts"), new RegExp(fn));
  }

  const pkg = JSON.parse(read("package.json"));
  assert.equal(pkg.scripts["build:ms-v1"], "tsc && vite build --mode store-safe");
  const msv1 = JSON.parse(read("src-tauri/tauri.msv1.conf.json"));
  assert.equal(msv1.build.beforeBuildCommand, "npm run build:ms-v1");
  assert.deepEqual(msv1.bundle.resources, [
    "tools/wireguard/*",
    "tools/cloudflared/*",
    "../plugins/configs/store-safe/*.json",
  ]);
  assert.match(pkg.scripts["tauri:build:ms-v1:windows"], /--config src-tauri\/tauri\.msv1\.conf\.json --features store-safe/);
  for (const file of fs.readdirSync("plugins/configs/store-safe")) {
    assert.doesNotMatch(read(`plugins/configs/store-safe/${file}`), /adult|porn|stash/i, file);
  }
});

test("adult-only surfaces are gated out of the store-safe edition", () => {
  const gated = {
    "src/components/tabs/MediaSourcesTab.tsx": /!IS_STORE_SAFE && <option value="adult">/,
    "src/components/tabs/PluginsTab.tsx": /IS_STORE_SAFE\s*\?\s*FULL_PLUGIN_REGISTRY\.filter\(\(plugin\) => !plugin\.tags\.includes\("adult"\)\)/,
    "src/components/tabs/HomeTab.tsx": /!IS_STORE_SAFE && selectedMedia\.media_type === "adult"/,
    "src/components/kodi/KodiHomeLayout.tsx": /!IS_STORE_SAFE \? \[\{ id: "adult"/,
    "src/components/tabs/AIDiagnosticsTab.tsx": /!IS_STORE_SAFE \|\| action\.progressTask !== "adult_metadata_gather"/,
  };
  assert.match(read("src/components/tabs/AIDiagnosticsTab.tsx"), /tracksAdultGather =\s*!IS_STORE_SAFE &&/);
  // Restored providers go through libraryProviders(), which store-safe filters first.
  assert.match(read("src/store/appStore.ts"), /metadataProviders: libraryProviders\(metadataProviders\)/);
  assert.match(read("src/store/appStore.ts"), /applyLibraryProviderPolicy\(filterStoreSafeProviders\(providers\)/);
  for (const [file, pattern] of Object.entries(gated)) {
    assert.match(read(file), pattern, file);
  }
});

test("the Store edition back end refuses adult sources, providers and commands", () => {
  assert.match(read("src-tauri/Cargo.toml"), /\[features\][^[]*store-safe = \[\]/);
  assert.match(read("src-tauri/src/edition.rs"), /STORE_SAFE: bool = cfg!\(feature = "store-safe"\)/);
  const guarded = {
    "src-tauri/src/lib.rs": [/if !edition::STORE_SAFE \{\s*match plugin_configs::ensure_adult_provider_configs/, /convert_entire_library_to_adult\([^)]*\) -> Result<serde_json::Value, String> \{\s*edition::ensure_adult_allowed\(\)\?/],
    // Issue 1 changed gather_adult_metadata to return a job start instead
    // of the full report; it still refuses first with ensure_adult_allowed.
    // Issue 2 deleted ai.rs's dead, never-routed gather_adult_metadata_assets
    // pass; the live adult gather is enrichment::gather_adult_metadata_work,
    // which also refuses first with ensure_adult_allowed (asserted here via
    // enrichment.rs, plus the post-scan adult pass in post_scan.rs).
    "src-tauri/src/enrichment.rs": [/pub async fn gather_adult_metadata\([^)]*\) -> Result<enrichment_jobs::EnrichmentJobStart, String> \{\s*crate::edition::ensure_adult_allowed\(\)\?/, /pub\(crate\) async fn gather_adult_metadata_work\([^)]*\) -> Result<AdultMetadataReport, String> \{\s*crate::edition::ensure_adult_allowed\(\)\?/],
    "src-tauri/src/ai.rs": [/ai_automation::ai_library_manage/],
    "src-tauri/src/post_scan.rs": [/crate::edition::ensure_adult_allowed\(\)\.is_ok\(\)/],
    "src-tauri/src/metadata_ext.rs": [/if crate::edition::STORE_SAFE \{\s*return Ok\(false\);/, /crate::edition::ensure_adult_allowed\(\)\?/],
    "src-tauri/src/db.rs": [/crate::edition::source_type_blocked\(&source_type\)/],
    "src-tauri/src/scanner.rs": [/crate::edition::source_type_blocked\(&source\.source_type\)/],
  };
  for (const [file, patterns] of Object.entries(guarded)) {
    for (const pattern of patterns) assert.match(read(file), pattern, file);
  }
});
