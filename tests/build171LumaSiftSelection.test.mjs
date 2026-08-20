import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";

const root = path.resolve(import.meta.dirname, "..");
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), "utf8");

test("Build 171 keeps owner-selected LumaSift file categories wired through Windows", () => {
  const engine = read("src-tauri/src/lumasift.rs");
  const scanner = read("src-tauri/src/scanner.rs");
  const screen = read("src/components/tabs/LumaSiftTab.tsx");

  assert.match(engine, /SUPPORTED_SELECTION_TYPES.*video.*audio.*document.*image/s);
  assert.match(engine, /"movie".*"video"/s);
  assert.match(engine, /"music".*"audio"/s);
  assert.match(engine, /"document".*"document"/s);
  assert.match(engine, /is_mp3[\s\S]*eq_ignore_ascii_case\("mp3"\)/);
  assert.match(engine, /start_lumasift_resolution[\s\S]*selected_types/);
  assert.match(scanner, /DOCUMENT_EXTS.*"docx".*"pdf"/s);
  assert.match(scanner, /Some\("document"\)/);
  assert.match(screen, /RESOLUTION_TYPES/);
  assert.match(screen, /MP3 audio/);
  assert.match(screen, /DOCX and PDF/);
  assert.match(screen, /start_lumasift_resolution", \{ selectedTypes \}/);
});

test("Build 171 preserves selected-category parity in iOS and Android companions", () => {
  const iosRoot = path.resolve(root, "..", "Cinavault-Server-Premium-Edition-iOS", "CinaVaultIOS");
  const androidRoot = path.resolve(root, "..", "cinavault-android", "app", "src", "main", "java", "com", "cinavault", "android");
  const iosView = fs.readFileSync(path.join(iosRoot, "LumaSiftView.swift"), "utf8");
  const iosApi = fs.readFileSync(path.join(iosRoot, "CinaVaultAPI.swift"), "utf8");
  const androidView = fs.readFileSync(path.join(androidRoot, "ui", "LumaSiftScreen.kt"), "utf8");
  const androidApi = fs.readFileSync(path.join(androidRoot, "network", "CinaVaultApi.kt"), "utf8");

  assert.match(iosView, /"video", "audio", "document", "image"/);
  assert.match(iosView, /MP3 AUDIO/);
  assert.match(iosView, /DOCX \+ PDF/);
  assert.match(iosApi, /LumaSiftStartBody\(selectedTypes: selectedTypes\)/);
  assert.match(androidView, /"MP3 AUDIO"/);
  assert.match(androidView, /"DOCX \+ PDF"/);
  assert.match(androidApi, /put\("selectedTypes", JSONArray\(selectedTypes\)\)/);
});
