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

function assertIncludes(text, expected, label = expected) {
  assert.ok(text.includes(expected), `${label} was not found`);
}

test("Build 140 routes user-facing metadata commands through metadata_ext at runtime", () => {
  const main = source("src-tauri/src/main.rs");
  const commandBlock = main.slice(
    main.indexOf("// Metadata commands route through metadata_ext"),
    main.indexOf("// Chapters"),
  );

  assertIncludes(commandBlock, "metadata_ext::fetch_metadata");
  assertIncludes(commandBlock, "metadata_ext::search_metadata");
  assertIncludes(commandBlock, "metadata_ext::check_media_item_metadata");
  assertIncludes(commandBlock, "metadata_ext::get_provider_status");
  assertIncludes(commandBlock, "metadata_ext::test_api_key");
  assertIncludes(commandBlock, "metadata_ext::set_api_key");
  assertIncludes(commandBlock, "metadata_ext::get_api_keys");
  assertIncludes(commandBlock, "metadata_ext::get_metadata_providers");

  assert.equal(commandBlock.includes("metadata::fetch_metadata"), false);
  assert.equal(commandBlock.includes("metadata::search_metadata"), false);
  assert.equal(commandBlock.includes("metadata::get_metadata_providers"), false);
});

test("Build 140 metadata extension handlers are real Tauri commands", () => {
  const metadataExt = source("src-tauri/src/metadata_ext.rs");
  const commandHandlers = [
    "get_metadata_providers",
    "fetch_metadata",
    "search_metadata",
    "check_media_item_metadata",
    "get_provider_status",
    "test_api_key",
    "set_api_key",
    "get_api_keys",
  ];

  for (const handler of commandHandlers) {
    const pattern = new RegExp(`#\\[tauri::command\\]\\s+pub(?: async)? fn ${handler}\\b`, "m");
    assert.match(metadataExt, pattern, `${handler} must be exported as a Tauri command`);
  }
});

test("Build 140 restored providers remain normalized and listed by extension source", () => {
  const metadataExt = source("src-tauri/src/metadata_ext.rs");

  for (const required of [
    "PGMA Modernized",
    "pgma_modernized_native_sidecar_metadata_bridge",
    "porn_site_nuxt",
    "Porn Site Nuxt",
    "PORN_SITE_NUXT_DEFAULT_BASE_URL",
    "normalize_provider_key",
  ]) {
    assertIncludes(metadataExt, required);
  }
});
