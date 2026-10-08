// Every switch in Advanced > Feature Matrix must change real behaviour: it is
// listed once, has a default both sides read, and some code outside the
// switch plumbing checks it.
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const read = (path) => readFileSync(join(ROOT, path), "utf8");
const defaults = JSON.parse(read("src/features/featureDefaults.json"));
const catalog = read("src/features/featureCatalog.ts");
const catalogKeys = [...catalog.matchAll(/\{ key: "([a-z_]+)"/g)].map((m) => m[1]);

function walk(dir, out = []) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) walk(path, out);
    else if (/\.(tsx?|rs)$/.test(name)) out.push(path);
  }
  return out;
}

const PLUMBING = new Set([
  "src/features/featureCatalog.ts",
  "src/features/featureFlags.ts",
  "src/components/tabs/AdvancedTab.tsx",
  "src-tauri/src/feature_flags.rs",
]);
const sources = [...walk(join(ROOT, "src")), ...walk(join(ROOT, "src-tauri/src"))]
  .map((path) => [relative(ROOT, path).split(sep).join("/"), readFileSync(path, "utf8")])
  .filter(([path]) => !PLUMBING.has(path) && !path.startsWith("src/features/panels/"));

test("catalog and defaults list the same switches once each", () => {
  assert.equal(new Set(catalogKeys).size, catalogKeys.length, "duplicate switch in catalog");
  assert.deepEqual([...catalogKeys].sort(), Object.keys(defaults).sort());
  assert.ok(catalogKeys.length >= 40);
});

test("every switch is read by the code it controls", () => {
  const unread = catalogKeys.filter((key) => {
    const check = new RegExp(
      `(useFeature|isFeatureOn|featureOn|is_enabled|is_enabled_state|feature_on)\\s*(<[^>]*>)?\\s*\\([^)]*["']${key}["']`,
    );
    return !sources.some(([, text]) => check.test(text));
  });
  assert.deepEqual(unread, [], `switches nothing reads: ${unread.join(", ")}`);
});
