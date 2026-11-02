// Issue 2 (g): the adult gather must not promise poster/chapter generation.
// The live gather saves provider posters (and, as a fallback, promotes an
// existing chapter frame); it generates neither. This test pins the
// corrected quick-action prompt and the new chapter-frame report field in
// the tab that Issue 2 touched, and fails if the old promise comes back.
import { readFileSync } from "node:fs";
import test from "node:test";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import path from "node:path";

const here = path.dirname(fileURLToPath(import.meta.url));
const tab = readFileSync(
  path.join(here, "..", "src", "components", "tabs", "AIDiagnosticsTab.tsx"),
  "utf8",
);

test("adult gather prompt no longer promises poster/chapter generation", () => {
  assert.equal(
    tab.includes("generate posters and chapter images"),
    false,
    "old prompt promising generation must not remain",
  );
  assert.equal(
    tab.includes(
      "Run adult metadata gather for installed providers and save provider posters and metadata",
    ),
    true,
    "corrected prompt must be present",
  );
});

test("adult gather report surfaces chapter-frame posters separately", () => {
  assert.equal(
    tab.includes("posters_from_chapter_frames?: number"),
    true,
    "report type must carry the Issue 2 chapter-frame field",
  );
  assert.equal(
    tab.includes("chapter-frame posters ${result.posters_from_chapter_frames || 0}"),
    true,
    "summary line must report chapter-frame posters",
  );
});
