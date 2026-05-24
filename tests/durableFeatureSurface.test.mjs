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
  assert.match(duplicateTab, /remove_duplicates/);
  assert.match(duplicateTab, /Select All/);
  assert.match(duplicateTab, /Delete All/);
  assert.match(duplicateTab, /Delete Selected Files/);
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

test("WD MyCloud remains a visible media-source path", () => {
  const mediaSources = read("src/components/tabs/MediaSourcesTab.tsx");
  const db = read("src-tauri/src/db.rs");

  assert.match(mediaSources, /WD MyCloud/);
  assert.match(mediaSources, /wd_mycloud/);
  assert.match(mediaSources, /wdmycloud:\/\//);
  assert.match(db, /source_type TEXT NOT NULL DEFAULT 'folder'/);
});

test("metadata provider tab exposes API key save and test controls", () => {
  const pluginsTab = read("src/components/tabs/PluginsTab.tsx");
  const metadata = read("src-tauri/src/metadata.rs");
  const main = read("src-tauri/src/main.rs");

  assert.match(pluginsTab, /get_api_keys/);
  assert.match(pluginsTab, /set_api_key/);
  assert.match(pluginsTab, /test_api_key/);
  assert.match(pluginsTab, /KEYED_METADATA_PROVIDERS/);
  assert.match(metadata, /pub fn set_api_key/);
  assert.match(metadata, /pub fn get_api_keys/);
  assert.match(metadata, /pub async fn test_api_key/);
  assert.match(main, /metadata::set_api_key/);
  assert.match(main, /metadata::get_api_keys/);
  assert.match(main, /metadata::test_api_key/);
});

test("PhoenixAdult remains an active local adult metadata provider", () => {
  const ai = read("src-tauri/src/ai.rs");
  const metadata = read("src-tauri/src/metadata.rs");
  const phoenix = read("src-tauri/src/phoenix_adult_provider.rs");

  assert.match(ai, /phoenix_adult_local_metadata/);
  assert.match(ai, /generated_screenshot_posters/);
  assert.match(metadata, /phoenixadult/);
  assert.match(metadata, /phoenix_adult_manifest_summary/);
  assert.match(phoenix, /8f97371f-8617-463c-9859-a33072182494/);
  assert.match(phoenix, /dc40637f-6ebd-4a34-b8a1-8799629120cf/);
  assert.match(phoenix, /FlareSolverrURL/);
  assert.match(phoenix, /DisableCaching/);
  assert.match(phoenix, /Jellyfin\.Plugin\.PhoenixAdult\.zip/);
  assert.doesNotMatch(ai, /provider_integration_pending.*phoenixadult/);
});

test("ThePornDB keeps the supplied plugin configuration defaults", () => {
  const ai = read("src-tauri/src/ai.rs");
  const metadata = read("src-tauri/src/metadata.rs");
  const tpdb = read("src-tauri/src/theporndb_provider.rs");

  assert.match(metadata, /theporndb_provider_manifest_summary/);
  assert.match(ai, /theporndb_provider_manifest_summary/);
  assert.match(tpdb, /MetadataAPIToken/);
  assert.match(tpdb, /UseFilePath/);
  assert.match(tpdb, /UseOSHash/);
  assert.match(tpdb, /ScenesImage/);
  assert.match(tpdb, /CollectionType/);
  assert.match(tpdb, /Missing From ThePornDB/);
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

test("all adult metadata providers are enabled by default", () => {
  const store = read("src/store/appStore.ts");

  assert.match(store, /id: "tpdb", name: "ThePornDB", category: "Adult", enabled: true/);
  assert.match(store, /id: "stashdb", name: "StashDB", category: "Adult", enabled: true/);
  assert.match(store, /id: "phoenixadult", name: "PhoenixAdult", category: "Adult", enabled: true/);
  assert.match(store, /id: "iafd", name: "IAFD", category: "Adult", enabled: true/);
  assert.match(store, /id: "porn_site_nuxt", name: "Porn Site Nuxt", category: "Adult", enabled: true/);
  assert.match(store, /metadata_selected_providers/);
});

test("built-in VPN and antivirus controls are real command surfaces", () => {
  const main = read("src-tauri/src/main.rs");
  const vpnb = read("src-tauri/src/vpnb.rs");
  const avb = read("src-tauri/src/avb.rs");
  const security = read("src/components/tabs/SecurityTab.tsx");

  assert.match(main, /vpnb::vpnb_status/);
  assert.match(main, /vpnb::vpnb_connect/);
  assert.match(main, /avb::avb_status/);
  assert.match(main, /avb::avb_scan_path/);
  assert.match(vpnb, /wireguard\.exe/);
  assert.match(vpnb, /wg-quick/);
  assert.match(avb, /Start-MpScan/);
  assert.match(avb, /Update-MpSignature/);
  assert.match(security, /vpnb_status/);
  assert.match(security, /avb_status/);
});

test("each media card exposes an isolated one-item metadata check action", () => {
  const homeTab = read("src/components/tabs/HomeTab.tsx");
  const ai = read("src-tauri/src/ai.rs");
  const main = read("src-tauri/src/main.rs");

  assert.match(homeTab, /onCheckMetadata/);
  assert.match(homeTab, /check_media_item_metadata/);
  assert.match(homeTab, /Check metadata/);
  assert.match(homeTab, /stopPropagation/);
  assert.match(ai, /pub async fn check_media_item_metadata/);
  assert.match(main, /ai::check_media_item_metadata/);
});

test("local poster files are renderable through the Tauri asset protocol", () => {
  const tauriConfig = JSON.parse(read("src-tauri/tauri.conf.json"));

  assert.equal(tauriConfig.app.security.assetProtocol.enable, true);
  assert.deepEqual(tauriConfig.app.security.assetProtocol.scope, {
    allow: ["**"],
    requireLiteralLeadingDot: false,
  });
  assert.match(tauriConfig.app.security.csp, /img-src[^;]*asset:/);
  assert.match(tauriConfig.app.security.csp, /img-src[^;]*http:\/\/asset\.localhost/);
  assert.match(tauriConfig.app.security.csp, /img-src[^;]*https:\/\/asset\.localhost/);
});
