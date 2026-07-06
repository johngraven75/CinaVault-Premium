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

const metadataHandlers = [
  "get_metadata_providers",
  "fetch_metadata",
  "search_metadata",
  "check_media_item_metadata",
  "get_provider_status",
  "test_api_key",
  "set_api_key",
  "get_api_keys",
];

test("Build 140 registers stable metadata wrapper commands at runtime", () => {
  const main = source("src-tauri/src/main.rs");
  const commandBlock = main.slice(
    main.indexOf("// Metadata commands delegate through metadata_ext"),
    main.indexOf("// Chapters"),
  );

  assertIncludes(commandBlock, "// Metadata commands delegate through metadata_ext");

  for (const handler of metadataHandlers) {
    assertIncludes(commandBlock, handler);
    assert.equal(
      commandBlock.includes(`metadata::${handler}`),
      false,
      `${handler} must not route through stale metadata::* commands`,
    );
    assert.equal(
      commandBlock.includes(`metadata_ext::${handler}`),
      false,
      `${handler} must be registered through the stable root wrapper`,
    );
  }
});

test("Build 140 root metadata wrappers delegate into metadata_ext", () => {
  const main = source("src-tauri/src/main.rs");

  for (const handler of metadataHandlers) {
    const wrapperPattern = new RegExp(`#\\[tauri::command\\]\\s+(?:async\\s+)?fn ${handler}\\b`, "m");
    const delegatePattern = new RegExp(`metadata_ext::${handler}\\s*\\(`, "m");

    assert.match(main, wrapperPattern, `${handler} must be a root Tauri command wrapper`);
    assert.match(main, delegatePattern, `${handler} must delegate to metadata_ext`);
  }
});

test("Build 140 metadata extension handlers remain real Tauri commands", () => {
  const metadataExt = source("src-tauri/src/metadata_ext.rs");

  for (const handler of metadataHandlers) {
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
