import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  FIRST_RUN_SETTING_KEY,
  FIRST_RUN_STEPS,
  buildKeylessProviders,
  buildSetupProviders,
  configuredProviderIds,
  initialKeyState,
  interpretTestResult,
  isFirstRunComplete,
  isLastStep,
  maskKey,
  nextStep,
  previousStep,
  shouldShowFirstRunSetup,
  validateApiKeyInput,
} from "../src/services/firstRunSetup.ts";
import {
  ADULT_KEYLESS_PROVIDERS,
  ADULT_SETUP_PROVIDERS,
} from "../src/services/firstRunAdultProviders.ts";

test("wizard walks welcome → metadata → vision → done and clamps at the ends", () => {
  assert.deepEqual([...FIRST_RUN_STEPS], ["welcome", "metadata", "vision", "done"]);
  assert.equal(nextStep("welcome"), "metadata");
  assert.equal(nextStep("metadata"), "vision");
  assert.equal(nextStep("vision"), "done");
  assert.equal(nextStep("done"), "done");
  assert.equal(previousStep("welcome"), "welcome");
  assert.equal(previousStep("done"), "vision");
  assert.equal(isLastStep("done"), true);
  assert.equal(isLastStep("vision"), false);
});

test("the setup is shown until first_run_setup_complete is \"true\"", () => {
  assert.equal(FIRST_RUN_SETTING_KEY, "first_run_setup_complete");
  assert.equal(shouldShowFirstRunSetup(null), true);
  assert.equal(shouldShowFirstRunSetup(undefined), true);
  assert.equal(shouldShowFirstRunSetup(""), true);
  assert.equal(shouldShowFirstRunSetup("false"), true);
  assert.equal(shouldShowFirstRunSetup("true"), false);
  assert.equal(isFirstRunComplete(" TRUE "), true);
});

test("store-safe edition never lists adult providers", () => {
  const full = buildSetupProviders(false, ADULT_SETUP_PROVIDERS).map((p) => p.id);
  assert.deepEqual(full, ["tpdb", "stashdb", "tmdb", "omdb", "fanart"]);
  const safe = buildSetupProviders(true, ADULT_SETUP_PROVIDERS);
  assert.deepEqual(safe.map((p) => p.id), ["tmdb", "omdb", "fanart"]);
  assert.ok(safe.every((p) => !p.adult));

  assert.deepEqual(
    buildKeylessProviders(false, ADULT_KEYLESS_PROVIDERS).map((p) => p.name),
    ["IAFD", "PGMA bridge", "TVMaze", "Cinemeta"],
  );
  assert.deepEqual(
    buildKeylessProviders(true, ADULT_KEYLESS_PROVIDERS).map((p) => p.name),
    ["TVMaze", "Cinemeta"],
  );
  // Non-adult entries passed in the adult slot are ignored either way.
  assert.equal(
    buildSetupProviders(false, [{ ...ADULT_SETUP_PROVIDERS[0], adult: false }]).length,
    3,
  );
});

test("the wizard component keeps adult provider names out of its own source", () => {
  const wizard = readFileSync(
    new URL("../src/components/setup/FirstRunSetup.tsx", import.meta.url),
    "utf8",
  );
  assert.doesNotMatch(wizard, /ThePornDB|StashDB/);
  assert.match(wizard, /IS_STORE_SAFE \? \[\] : ADULT_SETUP_PROVIDERS/);
  assert.match(wizard, /"run_library_enrichment", \{ renameFiles: false \}/);
  assert.match(wizard, /"set_api_key", \{ provider: providerId, apiKey/);
  assert.match(wizard, /createSerialSaver\(/);
  assert.match(wizard, /"test_api_key", \{/);
});

test("key validation, test interpretation and masking", () => {
  const tmdb = buildSetupProviders(true).find((p) => p.id === "tmdb");
  assert.equal(validateApiKeyInput(tmdb, "   ").ok, false);
  assert.match(validateApiKeyInput(tmdb, "abc def ghi jkl mno").message, /spaces/);
  assert.match(validateApiKeyInput(tmdb, "short").message, /too short/);
  assert.deepEqual(validateApiKeyInput(tmdb, " 0123456789abcdef0123 "), { ok: true, message: null });

  assert.equal(interpretTestResult(tmdb, { provider: "tmdb", valid: true }).status, "valid");
  assert.equal(interpretTestResult(tmdb, { provider: "tmdb", valid: false }).status, "invalid");
  assert.equal(interpretTestResult(tmdb, null).status, "invalid");

  assert.deepEqual([...configuredProviderIds({ tmdb: "ab...yz", omdb: "", TPDB: "****" })], ["tmdb", "tpdb"]);
  assert.deepEqual([...configuredProviderIds(null)], []);
  assert.equal(initialKeyState(true).status, "saved");
  assert.equal(initialKeyState(false).status, "empty");

  assert.equal(maskKey("abc"), "••••");
  assert.equal(maskKey("abcdefgh"), "ab••••gh");
});

test("serial saver keeps the newest key when an older save finishes last", async () => {
  const { createSerialSaver } = await import("../src/services/firstRunSetup.ts");
  const stored = new Map();
  const releases = [];
  // Each save waits until the test releases it, in an order the test chooses.
  const saver = createSerialSaver((id, value) =>
    new Promise((resolve) => releases.push(() => { stored.set(id, value); resolve(true); })),
  );
  const first = saver("tmdb", "old-key");
  const second = saver("tmdb", "new-key");
  await new Promise((r) => setImmediate(r));
  // Only the first save has started: the second waits for it.
  assert.equal(releases.length, 1);
  releases[0]();
  assert.deepEqual(await first, { saved: true, latest: false });
  await new Promise((r) => setImmediate(r));
  releases[1]();
  assert.deepEqual(await second, { saved: true, latest: true });
  assert.equal(stored.get("tmdb"), "new-key");
});

test("serial saver keeps going after a failed save", async () => {
  const { createSerialSaver } = await import("../src/services/firstRunSetup.ts");
  let calls = 0;
  const saver = createSerialSaver(async () => {
    calls += 1;
    if (calls === 1) throw new Error("keyring locked");
    return true;
  });
  await assert.rejects(saver("tpdb", "a"), /keyring locked/);
  assert.deepEqual(await saver("tpdb", "b"), { saved: true, latest: true });
});

test("enrichment results mention skipped adult providers only when skipped", async () => {
  const { adultProvidersSkippedNote } = await import("../src/services/firstRunSetup.ts");
  assert.equal(adultProvidersSkippedNote({ adult_providers_skipped: "PAYWALL:adult_metadata" }).includes("CinaVault Plus"), true);
  assert.equal(adultProvidersSkippedNote({ adult_providers_skipped: null }), "");
  assert.equal(adultProvidersSkippedNote(undefined), "");
});
