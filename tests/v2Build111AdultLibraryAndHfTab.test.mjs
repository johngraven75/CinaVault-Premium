import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";

const read = (path) => fs.readFileSync(path, "utf8");

test("entire-library conversion is CinaVault-only and preserves poster files", () => {
  const backend = read("src-tauri/src/lib.rs");
  assert.match(backend, /convert_entire_library_to_adult/);
  assert.match(backend, /UPDATE media_items SET media_type = 'adult', poster_path = NULL, backdrop_path = NULL/);
  assert.match(backend, /"poster_files_deleted": 0/);
  assert.match(backend, /enrichment::gather_adult_metadata\(state\)\.await/);
});

test("adult items never use standard or local metadata and artwork fallbacks", () => {
  const enrichment = read("src-tauri/src/enrichment.rs");
  assert.doesNotMatch(enrichment, /SourceKind::AdultVideo\s*=>\s*fetch_standard_metadata/);
  assert.match(enrichment, /source_kind != SourceKind::AdultVideo/);
  const gather = enrichment.match(/pub async fn gather_adult_metadata[\s\S]*?\n}\n\n#\[cfg\(test\)\]/)?.[0] || "";
  assert.doesNotMatch(gather, /local_sidecar_artwork_match|fetch_standard_metadata/);
});

test("Hugging Face model selection has a dedicated top-level tab", () => {
  assert.match(read("src/store/appStore.ts"), /"hf-models"/);
  assert.match(read("src/components/Sidebar.tsx"), /label: "HF Models"/);
  assert.match(read("src/App.tsx"), /"hf-models": HFModelsTab/);
  const tab = read("src/components/tabs/HFModelsTab.tsx");
  assert.match(tab, /Free · Public · Ungated/);
  assert.match(tab, /Reasoning models only/);
  assert.match(tab, /Save and Use Selected Model/);
});

test("AI agent exposes a cooperative backend Stop control", () => {
  assert.match(read("src-tauri/src/task_progress.rs"), /pub fn stop_ai_agent/);
  assert.match(read("src-tauri/src/task_progress.rs"), /AtomicBool/);
  assert.match(read("src/components/tabs/AIDiagnosticsTab.tsx"), /Stop AI Agent/);
  assert.match(read("src-tauri/src/enrichment.rs"), /task_progress::stop_requested\(\)/);
});

test("all Windows version sources are synchronized at v2.0.11 Build 1.11", () => {
  const manifest = JSON.parse(read("build-version.json"));
  assert.equal(manifest.semanticVersion, "2.0.11");
  assert.equal(manifest.displayBuild, "1.11");
  assert.equal(JSON.parse(read("package.json")).version, "2.0.11");
  assert.match(read("src-tauri/Cargo.toml"), /version = "2\.0\.11"/);
  assert.equal(JSON.parse(read("src-tauri/tauri.conf.json")).version, "2.0.11");
});
