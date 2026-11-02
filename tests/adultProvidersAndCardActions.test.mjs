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

test("Issue 5: HoloCard exposes a cornerMenu slot that stays stateless", async () => {
  const card = await read("src/components/holo/HoloCard.tsx");
  assert.match(card, /cornerMenu\?: ReactNode/);
  assert.match(
    card,
    /{cornerMenu && <div className="holo-card__corner">{cornerMenu}<\/div>}/,
  );
  // The holoUi contract: HoloCard itself never owns state, so the menu's open
  // state must live in the slotted component, not here.
  assert.doesNotMatch(card, /useState/);
  const css = await read("src/styles/holo-cinema.css");
  assert.match(css, /\.holo-card__corner\s*\{[^}]*pointer-events:\s*auto/);
  assert.match(css, /\.holo-card__body:has\(> \.holo-card__corner\) \.holo-card__badges/);
  assert.match(css, /\.holo-card__menu\s*\{/);
  assert.match(css, /\.holo-card__menu-item/);
});

test("Issue 5: the card corner menu offers Unmatch, Match and Lookup with guards", async () => {
  const menu = await read("src/components/CardCornerMenu.tsx");
  assert.match(menu, /aria-haspopup="menu"/);
  assert.match(menu, /aria-expanded={open}/);
  assert.match(menu, /role="menu"/);
  assert.match(menu, /role="menuitem"/);
  assert.match(menu, /> Unmatch/);
  assert.match(menu, /> Match \(auto\)/);
  assert.match(menu, /> Lookup…/);
  // The card is a role="button": menu clicks must not also select the card.
  assert.match(menu, /event\.stopPropagation\(\)/);
  // Close paths: select, Escape, and outside pointer press.
  assert.match(menu, /event\.key === "Escape"/);
  assert.match(menu, /document\.addEventListener\("mousedown"/);
  // Busy/checking guards: the button and every item disable together.
  assert.match(menu, /disabled={disabled}/);
  assert.match(menu, /if \(disabled\) setOpen\(false\)/);

  const home = await read("src/components/tabs/HomeTab.tsx");
  assert.match(home, /import CardCornerMenu from "\.\.\/CardCornerMenu"/);
  assert.match(home, /cornerMenu={/);
  assert.match(home, /disabled={checking \|\| busy}/);
  assert.match(home, /onUnmatch={onUnmatch}/);
  assert.match(home, /onMatch={onCheckMetadata}/);
  assert.match(home, /onLookup={onLookup}/);
  // The existing bottom hover actions stay: the menu adds entry points, it
  // does not move the only Unmatch/Delete controls.
  assert.match(home, /aria-label={`Unmatch \${item\.title}, keeps the file`}/);
});

test("Issue 5: the Lookup dialog searches, ranks with posterIdentification, and applies", async () => {
  const dialog = await read("src/components/MetadataLookupDialog.tsx");
  // Provider list from the verified backend commands, edition-filtered.
  assert.match(dialog, /invoke<BackendProvider\[\]>\("get_metadata_providers"\)/);
  assert.match(dialog, /invoke<ProviderStatus>\("get_provider_status"\)/);
  assert.match(dialog, /filterStoreSafeProviders/);
  assert.match(dialog, /applyLibraryProviderPolicy/);
  // Search through the verified command, same arguments as searchAndIdentify.
  assert.match(dialog, /invoke<unknown>\("search_metadata"/);
  assert.match(dialog, /mediaType: item\.media_type/);
  assert.match(dialog, /apiKey: null/);
  // Query defaults to the card title; ranking reuses the autopilot helpers.
  assert.match(dialog, /useState\(item\.title\)/);
  assert.match(dialog, /normalizeSearchResults\(searchProvider, raw\)/);
  assert.match(dialog, /scoreCandidates\(/);
  // Apply maps the candidate to the new command; fields the candidate does
  // not carry are sent as null so the backend preserves them.
  assert.match(dialog, /invoke<ApplyMatchResult>\("metadata_apply_match_item"/);
  assert.match(dialog, /poster_path: candidate\.posterUrl/);
  assert.match(dialog, /backdrop_path: null/);
  assert.match(dialog, /genre: null/);
  assert.match(dialog, /rating: null/);
  assert.match(dialog, /result\.updated_item/);

  const identification = await read("src/services/posterIdentification.ts");
  assert.match(identification, /export function normalizeSearchResults/);
  assert.match(identification, /export function scoreCandidates/);

  const home = await read("src/components/tabs/HomeTab.tsx");
  assert.match(home, /import MetadataLookupDialog from "\.\.\/MetadataLookupDialog"/);
  assert.match(home, /<MetadataLookupDialog/);
  assert.match(home, /onApplied={handleLookupApplied}/);
  assert.match(home, /const handleLookupApplied[\s\S]{0,400}applyUpdatedMediaItem\(updated\)/);
  // Detail panel gets a Lookup entry point too; the list rows do not get a
  // second menu — Lookup there is reached via the detail panel / card view.
  assert.match(home, /aria-label={`Lookup metadata for \${selectedMedia\.title}`}/);
});

test("Issue 5: metadata_apply_match_item is registered and maps candidate fields", async () => {
  const lib = await read("src-tauri/src/lib.rs");
  assert.match(lib, /library_item_actions::metadata_apply_match_item/);
  const actions = await read("src-tauri/src/library_item_actions.rs");
  assert.match(actions, /pub fn metadata_apply_match_item/);
  assert.match(actions, /update_media_metadata_data/);
  // The helper does not cover backdrop_path, so the command writes it directly.
  assert.match(actions, /UPDATE media_items SET backdrop_path = \?1 WHERE id = \?2/);
  // The frontend refreshes from the returned row, like Check metadata.
  assert.match(actions, /"updated_item": updated/);
  // Empty candidates and missing items never write.
  assert.match(actions, /No metadata fields to apply/);
  assert.match(actions, /Library item not found/);
  // Edition gate: no blanket adult gate (that would refuse movie/TV matches
  // in the store-safe edition, unlike the siblings) — adult items in a
  // store-safe build are refused through the edition module's own helper.
  assert.match(actions, /source_type_blocked\(&media_type\)/);
  assert.match(actions, /STORE_SAFE_REFUSAL/);
  assert.doesNotMatch(actions, /edition::ensure_adult_allowed\(\)\?/);
});

test("Issue 5: the Kodi skin card is explicitly NOT given the corner menu", async () => {
  // KodiHomeLayout owns its own check handler and has no unmatch/delete
  // plumbing, so threading the menu through it is a separate change, not a
  // small reuse. This test locks the boundary in: the holo card menu must not
  // silently appear (or half-appear) in the Kodi skin.
  const kodi = await read("src/components/kodi/KodiHomeLayout.tsx");
  assert.doesNotMatch(kodi, /CardCornerMenu/);
  assert.doesNotMatch(kodi, /MetadataLookupDialog/);
  const home = await read("src/components/tabs/HomeTab.tsx");
  assert.match(home, /currentTheme\?\.startsWith\("kodi_"\)/);
});
