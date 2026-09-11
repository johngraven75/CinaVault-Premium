import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const read = (path) => readFile(new URL(`../${path}`, import.meta.url), "utf8");

test("adult gather quick action calls the dedicated adult-only command", async () => {
  const source = await read("src/components/tabs/AIDiagnosticsTab.tsx");
  assert.match(source, /runNow:\s*\(\)\s*=>\s*invoke\("gather_adult_metadata"\)/);
  assert.match(source, /invoke<AdultMetadataGatherResult>\("gather_adult_metadata"\)/);
});

test("adult provider controls save masked credentials and enabled state through backend commands", async () => {
  const source = await read("src/components/tabs/AIDiagnosticsTab.tsx");
  assert.match(source, /"get_adult_provider_settings"/);
  assert.match(source, /"save_adult_provider_settings"/);
  assert.match(source, /"set_api_key"/);
  assert.match(source, /"test_api_key"/);
  assert.match(source, /Credentials remain masked/);
});

test("adult media details request generated chapter artwork", async () => {
  const source = await read("src/components/tabs/HomeTab.tsx");
  assert.match(source, /selectedMedia\.media_type === "adult"/);
  assert.match(source, /invoke<ChapterThumb\[\]>\("get_chapter_thumbs"/);
});
