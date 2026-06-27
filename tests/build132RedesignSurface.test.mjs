import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, "..");

function source(path) {
  return readFileSync(resolve(root, path), "utf8");
}

function expectIncludes(file, requiredValues) {
  const text = source(file);
  for (const required of requiredValues) {
    assert.ok(text.includes(required), `${file} should include ${required}`);
  }
  return text;
}

test("Build 139 release metadata is aligned with package version", () => {
  const pkg = JSON.parse(source("package.json"));
  assert.equal(pkg.version, "1.0.139");

  const notes = source("releases/build-139/RELEASE-NOTES.md");
  assert.match(notes, /Build 139/i);
  assert.match(notes, /v?1\.0\.139/i);
});

test("Build and Test workflow runs frontend surface coverage", () => {
  const workflow = expectIncludes(".github/workflows/build-and-test.yml", [
    "npm ci",
    "npm run build",
    "npm run test:build132",
    "cargo test --manifest-path src-tauri/Cargo.toml scanner::tests -- --nocapture",
    "cargo test --manifest-path src-tauri/Cargo.toml metadata_posting_tests -- --nocapture",
  ]);
  assert.match(workflow, /node-version:\s*\[18\.x,\s*20\.x\]/);
});

test("Build 132 app shell keeps startup, persistence, plugin, and wheel-scroll behavior", () => {
  expectIncludes("src/App.tsx", [
    "Build 132 Futuristic Application Shell",
    "app-shell",
    "app-shell-orb",
    "app-shell-noise",
    "pluginEngine.initialize",
    "applyTheme(currentTheme)",
    "restorePersistedState(readLocalPersistedState())",
    "saveAllSettingsToBackend",
    "getWheelDeltaPixels",
    "getWheelScrolledTop",
    "AnimatePresence mode=\"wait\"",
  ]);
});

test("Build 132 sidebar keeps every core destination and collapse control", () => {
  expectIncludes("src/components/Sidebar.tsx", [
    "Build 132 Futuristic Sidebar Navigation",
    "type LucideIcon",
    "sidebar-active-panel",
    "sidebar-active-rail",
    "toggleSidebar",
    "home",
    "sources",
    "downloads",
    "livetv",
    "server",
    "security",
    "remote",
    "advanced",
    "cloud",
    "plugins",
    "ai",
    "settings",
  ]);
});

test("Build 139 header keeps HUD identity, search, clock, fullscreen, and notifications", () => {
  expectIncludes("src/components/Header.tsx", [
    "Build 139 Cyber HUD Command Header",
    "TAB_META",
    "Quantum Grid Active",
    "CinaVault B139",
    "Hyper-Neon Fusion",
    "setSearchQuery",
    "toLocaleTimeString",
    "toggleFullscreen",
    "requestFullscreen",
    "getUnreadStatusMessages",
    "devicePixelRatio",
    "requestAnimationFrame(draw)",
  ]);
});

test("Kodi-inspired CinaVault skins are selectable in Settings", () => {
  expectIncludes("src/themes.ts", [
    "kodi_estuary_cinema",
    "CinaVault Estuary",
    "kodi_aeon_nox_lux",
    "CinaVault Aeon Nox",
    "kodi_arctic_zephyr",
    "CinaVault Arctic Zephyr",
    "kodi_titan_bingie_stream",
    "CinaVault Titan Bingie",
    "kodi_amber_home",
    "CinaVault Amber",
    "origin: \"Kodi\"",
  ]);

  expectIncludes("src/components/tabs/SettingsTab.tsx", [
    "Themes & Skins",
    "Kodi Skin",
    "theme.description",
    "THEME_PRESETS.length",
  ]);
});

test("PGMA and Porn Site Nuxt remain exposed as metadata providers", () => {
  const store = source("src/store/appStore.ts");
  const backendFiles = [
    "src-tauri/src/metadata_ext.rs",
    "src-tauri/src/adult_site_provider.rs",
    "src-tauri/src/pgma_bridge.rs",
    "src-tauri/src/main.rs",
    "src-tauri/src/lib.rs",
  ];

  const backend = backendFiles
    .map((file) => {
      try {
        return source(file);
      } catch {
        return "";
      }
    })
    .join("\n");

  for (const required of [
    "pgma",
    "PGMA Modernized",
    "porn_site_nuxt",
    "Porn Site Nuxt",
  ]) {
    assert.ok(store.includes(required), `store should include ${required}`);
    assert.ok(backend.includes(required), `Tauri backend should include ${required}`);
  }

  assert.match(
    backend,
    /fetch_metadata|get_metadata_providers|metadata provider|MetadataProvider/i,
    "Tauri backend should include metadata provider handling",
  );

  assert.match(
    backend,
    /find_local_candidates|pgma/i,
    "Tauri backend should include PGMA provider or candidate handling",
  );
});

test("PR 12 guards stale persisted tabs in store and header metadata lookup", () => {
  const store = expectIncludes("src/store/appStore.ts", [
    "const VALID_TAB_IDS",
    "function isTabId(value: string): value is TabId",
    "activeTab = isTabId(value) ? value : \"home\"",
  ]);
  const header = expectIncludes("src/components/Header.tsx", [
    "const activeMeta = TAB_META[activeTab] ?? TAB_META.home",
  ]);

  assert.doesNotMatch(store, /activeTab\s*=\s*value\s+as\s+TabId/);
  assert.doesNotMatch(header, /const\s+activeMeta\s*=\s*TAB_META\[activeTab\]\s*;/);
});

test("PR 12 empty and filtered libraries do not fall back to demo hero records", () => {
  const home = expectIncludes("src/components/tabs/HomeTab.tsx", [
    "const heroItem = selectedMedia || filteredItems[0] || null",
    "Vault Empty / Awaiting Scan",
    "No Media Found",
    "Refresh Library",
    "filteredItems.length === 0",
    "Add media sources and scan to populate the holographic vault.",
  ]);

  assert.doesNotMatch(home, /selectedMedia\s*\|\|\s*filteredItems\[0\]\s*\|\|\s*DEMO_ITEMS\[0\]/);
  assert.match(home, /catch\s*\{[\s\S]*setMediaItems\(DEMO_ITEMS\)/);
});

test("PR 12 media controls keep safe playback, metadata, verification, and low-motion paths", () => {
  expectIncludes("src/components/tabs/HomeTab.tsx", [
    "const reduceCardMotion = filteredItems.length > 500",
    "if (!canPlayMediaItem(item))",
    "Quick Play skipped",
    "if (!item.id)",
    "Verification skipped",
    "Metadata scan skipped",
    "invoke<any>(\"check_media_item_metadata\", { id: item.id })",
    "applyUpdatedMediaItem(result.updated_item)",
    "Quick Play",
    "Check Metadata",
    "setLibraryView(\"card\")",
    "setLibraryView(\"list\")",
    "setCardStyle(\"poster\")",
    "setCardStyle(\"disc\")",
    "setCardStyle(\"banner\")",
    "type=\"range\"",
    "setTitleInitialFilter(letter)",
  ]);
});

test("PR 12 AI Agent quick actions are wired to backend commands and progress tracking", () => {
  expectIncludes("src/components/tabs/AIDiagnosticsTab.tsx", [
    "const quickActions: QuickAction[] = [",
    "Network Diagnostics",
    "Check Sources",
    "Check Providers",
    "Post Metadata & Posters",
    "runNow: runBulkMetadataPost",
    "invoke<SingleItemMetadataCheckResult>(\"check_media_item_metadata\", { id: item.id })",
    "Enrich Library Metadata",
    "invoke(\"run_library_enrichment\", { renameFiles: false })",
    "Apply Embedded Titles",
    "invoke(\"apply_embedded_titles\")",
    "Enrich + Normalize Filenames",
    "invoke(\"run_library_enrichment\", { renameFiles: true })",
    "Adult Metadata Gather",
    "progressTask: \"adult_metadata_gather\"",
    "runQuickAction(action)",
    "disabled={aiProcessing}",
    "handleTrackedResult(action.label, action.q, result)",
    "refreshLoadedLibraryPage()",
  ]);
});

test("PR 12 Cyber HUD styles stay scoped away from legacy cv button variants", () => {
  const styles = expectIncludes("src/styles/cyber-hud.css", [
    ".cyber-button",
    ".cyber-button.is-amber",
    ".cyber-select",
    ".library-row-metadata-action",
  ]);

  assert.doesNotMatch(styles, /\.cv-btn-primary\s*\{/);
  assert.doesNotMatch(styles, /\.cv-btn-danger\s*\{/);
  assert.doesNotMatch(styles, /\.cv-btn-gold\s*\{/);
});

test("PR 12 staged E2E workflow remains manual until a real browser harness is committed", () => {
  const workflow = source(".github/workflows/real-user-e2e-agent.yml");
  const smoke = source("tests/e2e/smoke.spec.mjs");

  assert.match(workflow, /workflow_dispatch:/);
  assert.doesNotMatch(workflow, /pull_request:/);
  assert.match(workflow, /npm run build/);
  assert.match(workflow, /npm run test:build132/);
  assert.match(workflow, /Browser probe files must be committed before enabling pull_request gating\./);
  assert.match(smoke, /@playwright\/test/);
});
