import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  NOT_IN_USE_LABEL,
  applyLibraryProviderPolicy,
  providerInUse,
} from "../src/services/libraryMetadataPolicy.ts";

const PROVIDERS = [
  { id: "tmdb", name: "TMDb", category: "Movies & TV", enabled: true },
  { id: "omdb", name: "OMDb", category: "Movies & TV", enabled: true },
  { id: "tmdb_images", name: "TheMovieDB Images", category: "Artwork", enabled: true },
  { id: "theporndb", name: "ThePornDB", category: "Adult", enabled: true },
  { id: "stashdb", name: "StashDB", category: "Adult", enabled: true },
];

test("full edition keeps only adult providers in use", () => {
  const result = applyLibraryProviderPolicy(PROVIDERS, false);
  const active = result.filter((p) => p.enabled && p.inUse).map((p) => p.id);
  assert.deepEqual(active, ["theporndb", "stashdb"]);
  for (const id of ["tmdb", "omdb", "tmdb_images"]) {
    const provider = result.find((p) => p.id === id);
    assert.equal(provider.enabled, false, id);
    assert.equal(provider.inUse, false, id);
  }
});

test("a restored or toggled regular provider cannot be switched back on", () => {
  const toggled = PROVIDERS.map((p) => ({ ...p, enabled: true, inUse: true }));
  const result = applyLibraryProviderPolicy(toggled, false);
  assert.equal(result.find((p) => p.id === "tmdb").enabled, false);
  assert.equal(result.find((p) => p.id === "omdb").enabled, false);
});

test("store-safe edition keeps regular providers", () => {
  assert.equal(providerInUse({ category: "Movies & TV" }, true), true);
  const result = applyLibraryProviderPolicy(PROVIDERS, true);
  assert.ok(result.every((p) => p.enabled && p.inUse));
});

test("store and plugins tab route providers through the policy", () => {
  const store = readFileSync(new URL("../src/store/appStore.ts", import.meta.url), "utf8");
  assert.match(store, /applyLibraryProviderPolicy\(filterStoreSafeProviders\(providers\), IS_STORE_SAFE\)/);
  assert.doesNotMatch(store, /metadataProviders: filterStoreSafeProviders\(/);
  const tab = readFileSync(new URL("../src/components/tabs/PluginsTab.tsx", import.meta.url), "utf8");
  assert.match(tab, /disabled=\{provider\.inUse === false\}/);
  assert.equal(NOT_IN_USE_LABEL, "Not in use");
});

test("the Rust scan path gates every regular provider call", () => {
  const read = (file) =>
    readFileSync(new URL(`../src-tauri/src/${file}`, import.meta.url), "utf8");
  assert.match(read("enrichment.rs"), /SourceKind::StandardVideo\s+if crate::library_metadata_policy::standard_providers_enabled\(\)/);
  assert.match(read("enrichment.rs"), /retain_allowed_keys\(&mut keys\)/);
  assert.match(read("metadata_enrichment_runtime.rs"), /if !crate::library_metadata_policy::standard_providers_enabled\(\) \{\s+return Ok\(KeylessPrepassReport::default\(\)\);/);
  assert.match(read("metadata_guard.rs"), /else if crate::library_metadata_policy::standard_providers_enabled\(\) \{\s+let \(tmdb, omdb\)/);
  assert.match(read("metadata_ext.rs"), /ensure_provider_allowed\(&provider\)\?/);
  assert.match(read("ai.rs"), /retain_allowed_keys\(&mut keys\)/);
  for (const file of ["collections.rs", "content_rating.rs"]) {
    assert.match(read(file), /provider_allowed\("tmdb"\)/, file);
  }
  assert.match(read("metadata_bridge.rs"), /provider_allowed\("tmdb_images"\)/);
});

test("the single-title lookup falls back to every enabled adult provider", () => {
  const read = (file) =>
    readFileSync(new URL(`../src-tauri/src/${file}`, import.meta.url), "utf8");
  const guard = read("metadata_guard.rs");
  assert.match(guard, /enabled_extra_adult_provider_values\(&db\)/);
  assert.match(guard, /fetch_adult_metadata_for_batch\(&client, &extra_adult/);
  const ext = read("metadata_ext.rs");
  assert.match(ext, /EXTRA_ADULT_PROVIDER_KEYS: &\[&str\] =\s*&\["porn_site_nuxt", "iafd", "phoenixadult", "pgma"\]/);
  assert.match(read("lib.rs"), /metadata_ext::enable_all_adult_providers_once\(&database\)/);
});
