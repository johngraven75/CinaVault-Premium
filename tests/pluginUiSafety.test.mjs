import test from "node:test";
import assert from "node:assert/strict";

import {
  getMetadataProviderInitials,
  matchesPluginSearch,
  sanitizeMetadataProviders,
} from "../src/utils/pluginUiSafety.ts";

test("sanitizeMetadataProviders keeps defaults and repairs malformed persisted providers", () => {
  const defaults = [
    { id: "tmdb", name: "TMDb", category: "Movies & TV", enabled: true },
    { id: "theporndb", name: "ThePornDB", category: "Adult", enabled: false },
  ];

  const persisted = [
    { id: "tmdb", name: "TMDb", category: "Movies & TV", enabled: false },
    { id: "theporndb", category: "Adult", enabled: true },
    { id: "", name: "", category: "", enabled: true },
  ];

  assert.deepEqual(sanitizeMetadataProviders(persisted, defaults), [
    { id: "tmdb", name: "TMDb", category: "Movies & TV", enabled: false },
    { id: "theporndb", name: "ThePornDB", category: "Adult", enabled: true },
  ]);
});

test("getMetadataProviderInitials never throws on missing provider names", () => {
  assert.equal(getMetadataProviderInitials("PhoenixAdult"), "PH");
  assert.equal(getMetadataProviderInitials(""), "??");
  assert.equal(getMetadataProviderInitials(undefined), "??");
});

test("matchesPluginSearch tolerates missing description and tags", () => {
  const plugin = {
    name: "ThePornDB",
    description: undefined,
    tags: undefined,
  };

  assert.equal(matchesPluginSearch(plugin, "porn"), true);
  assert.equal(matchesPluginSearch(plugin, "metadata"), false);
  assert.equal(matchesPluginSearch(plugin, ""), true);
});
