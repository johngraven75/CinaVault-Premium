import test from "node:test";
import assert from "node:assert/strict";

import { shouldRefreshLibraryAfterMetadataResult } from "../src/utils/metadataResults.ts";

test("adult metadata gather results refresh the loaded library page", () => {
  assert.equal(
    shouldRefreshLibraryAfterMetadataResult({ type: "adult_metadata_gather", status: "success" }),
    true,
  );
});

test("generic inference results do not force a library refresh", () => {
  assert.equal(
    shouldRefreshLibraryAfterMetadataResult({ type: "inference", status: "success" }),
    false,
  );
});
