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

function requireAll(sourceText, requiredValues) {
  for (const required of requiredValues) {
    assert.match(sourceText, new RegExp(required.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
  }
}

test("Build 140 app shell keeps startup, persistence, plugin, and wheel-scroll behavior", () => {
  const app = source("src/App.tsx");

  requireAll(app, [
    "Build 140 Futuristic Application Shell",
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

test("Build 140 sidebar keeps every core destination and collapse control", () => {
  const sidebar = source("src/components/Sidebar.tsx");

  requireAll(sidebar, [
    "Build 140 Futuristic Sidebar Navigation",
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

test("Build 140 header keeps HUD identity, search, clock, fullscreen, and notifications", () => {
  const header = source("src/components/Header.tsx");

  requireAll(header, [
    "Build 140 Cyber HUD Command Header",
    "TAB_META",
    "Quantum Grid Active",
    "CinaVault B140",
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
  const themes = source("src/themes.ts");
  const settings = source("src/components/tabs/SettingsTab.tsx");

  requireAll(themes, [
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

  requireAll(settings, [
    "Themes & Skins",
    "Kodi Skin",
    "theme.description",
    "THEME_PRESETS.length",
  ]);
});

test("PGMA and Porn Site Nuxt remain exposed as metadata providers", () => {
  const store = source("src/store/appStore.ts");
  const backend = source("src-tauri/src/metadata_ext.rs");
  const main = source("src-tauri/src/main.rs");

  for (const required of [
    "pgma",
    "PGMA Modernized",
    "porn_site_nuxt",
    "Porn Site Nuxt",
  ]) {
    assert.match(store, new RegExp(required.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
    assert.match(backend, new RegExp(required.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
  }

  requireAll(main, [
    "mod adult_site_provider;",
    "mod metadata_ext;",
    "metadata_ext::fetch_metadata",
    "metadata_ext::get_metadata_providers",
    "pgma_bridge::find_local_candidates",
  ]);
});
