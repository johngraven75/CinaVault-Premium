import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const read = (path) => readFile(new URL(`../${path}`, import.meta.url), "utf8");

test("adult gather runs the full enabled adult chain, not just ThePornDB and StashDB", async () => {
  const source = await read("src-tauri/src/enrichment.rs");
  // The keyless adult providers are loaded and used as a fallback.
  assert.match(source, /enabled_extra_adult_provider_values/);
  assert.match(source, /fetch_adult_metadata_for_batch/);
  // The old "unavailable in this build" dead-end for IAFD/Phoenix is gone.
  assert.doesNotMatch(source, /unavailable in this build/);
});

test("the one-shot adult migration is versioned and never clobbers a user choice", async () => {
  const source = await read("src-tauri/src/metadata_ext.rs");
  assert.match(source, /adult_metadata_all_enabled_v2/);
  assert.match(source, /ADULT_PROVIDERS_USER_ADJUSTED_MARKER/);
  // Saving a provider list records that the user adjusted it.
  assert.match(
    source,
    /set_setting_data\(ADULT_PROVIDERS_USER_ADJUSTED_MARKER, "true"\)/,
  );
});

test("the per-item action commands are registered with Tauri", async () => {
  const source = await read("src-tauri/src/lib.rs");
  assert.match(source, /library_item_actions::metadata_unmatch_item/);
  assert.match(source, /library_item_actions::library_delete_item/);
});

test("each library card exposes Check metadata, Unmatch and Delete, wired to the right commands", async () => {
  const source = await read("src/components/tabs/HomeTab.tsx");
  // The three per-item backend calls the card fires.
  assert.match(source, /invoke<MetadataCheckResult>\("check_media_item_metadata"/);
  assert.match(source, /invoke<\{ fallback_title\?: string \}>\("metadata_unmatch_item"/);
  assert.match(source, /invoke<\{ file_removed\?: boolean \}>\("library_delete_item"/);
  // The delete command carries the remove-file choice.
  assert.match(source, /"library_delete_item",\s*\{\s*id: item\.id,\s*removeFile,/);
  // The card renders all three actions with accessible labels.
  assert.match(source, /aria-label=\{`Unmatch \$\{item\.title\}, keeps the file`\}/);
  assert.match(source, /aria-label=\{`Delete \$\{item\.title\}`\}/);
  // Delete is a two-step choice defaulting to library-only.
  assert.match(source, /Remove from library only/);
  assert.match(source, /Also send the file to trash/);
  assert.match(source, /handleConfirmDelete\(pendingDelete, false\)/);
  assert.match(source, /handleConfirmDelete\(pendingDelete, true\)/);
});

test("unmatch confirms and keeps the file; delete never hard-deletes", async () => {
  const home = await read("src/components/tabs/HomeTab.tsx");
  assert.match(home, /Unmatch "\$\{item\.title\}" and keep the file\?/);
  const actions = await read("src-tauri/src/library_item_actions.rs");
  // File removal goes through the OS trash, never a hard fs delete.
  assert.match(actions, /trash::delete/);
  assert.doesNotMatch(actions, /fs::remove_file|remove_file\(/);
});

test("the Delete key triggers delete on a focused library card", async () => {
  const holo = await read("src/components/holo/HoloCard.tsx");
  assert.match(holo, /onDeleteKey/);
  assert.match(holo, /event\.key === "Delete"/);
});
