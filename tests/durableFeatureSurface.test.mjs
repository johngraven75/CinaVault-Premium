import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

function read(path) {
  return readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
}

test("duplicate finder and deleter stay visible in the app shell", () => {
  const app = read("src/App.tsx");
  const store = read("src/store/appStore.ts");
  const sidebar = read("src/components/Sidebar.tsx");
  const header = read("src/components/Header.tsx");
  const duplicateTab = read("src/components/tabs/DuplicateToolsTab.tsx");

  assert.match(store, /"duplicates"/);
  assert.match(app, /DuplicateToolsTab/);
  assert.match(sidebar, /Duplicate Finder/);
  assert.match(header, /Duplicate Finder/);
  assert.match(duplicateTab, /find_duplicates/);
  assert.match(duplicateTab, /remove_duplicate/);
});

test("Synology QuickConnect login remains a visible media-source path", () => {
  const cloudNas = read("src/components/tabs/CloudNASTab.tsx");
  const mediaSources = read("src/components/tabs/MediaSourcesTab.tsx");

  assert.match(cloudNas, /Synology QuickConnect/);
  assert.match(cloudNas, /synology_quickconnect/);
  assert.match(cloudNas, /quickConnectId/);
  assert.match(mediaSources, /Synology QuickConnect/);
  assert.match(mediaSources, /synology_quickconnect/);
});

test("PhoenixAdult remains an active local adult metadata provider", () => {
  const ai = read("src-tauri/src/ai.rs");
  const metadata = read("src-tauri/src/metadata.rs");

  assert.match(ai, /phoenix_adult_local_metadata/);
  assert.match(ai, /generated_screenshot_posters/);
  assert.match(metadata, /phoenixadult/);
  assert.doesNotMatch(ai, /provider_integration_pending.*phoenixadult/);
});

test("Nuxt porn-site bundle remains an active adult metadata provider", () => {
  const ai = read("src-tauri/src/ai.rs");
  const metadata = read("src-tauri/src/metadata.rs");
  const store = read("src/store/appStore.ts");

  assert.match(ai, /fetch_porn_site_nuxt_metadata/);
  assert.match(ai, /porn_site_nuxt/);
  assert.match(metadata, /Porn Site Nuxt/);
  assert.match(store, /Porn Site Nuxt/);
});
