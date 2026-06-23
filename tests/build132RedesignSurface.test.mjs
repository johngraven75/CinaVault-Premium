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

test("Build 132 app shell keeps startup, persistence, plugin, and wheel-scroll behavior", () => {
  const app = source("src/App.tsx");

  for (const required of [
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
  ]) {
    assert.match(app, new RegExp(required.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
  }
});

test("Build 132 sidebar keeps every core destination and collapse control", () => {
  const sidebar = source("src/components/Sidebar.tsx");

  for (const required of [
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
  ]) {
    assert.match(sidebar, new RegExp(required.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
  }
});

test("Build 132 header keeps command identity, search, clock, fullscreen, and notifications", () => {
  const header = source("src/components/Header.tsx");

  for (const required of [
    "Build 132 Cinematic Command Header",
    "Build 132 Interface",
    "TAB_SUBTITLES",
    "setSearchQuery",
    "toLocaleTimeString",
    "toggleFullscreen",
    "requestFullscreen",
    "Command Feed",
    "getUnreadStatusMessages",
    "devicePixelRatio",
    "requestAnimationFrame(draw)",
  ]) {
    assert.match(header, new RegExp(required.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
  }
});
