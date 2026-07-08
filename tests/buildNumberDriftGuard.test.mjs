import test from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, extname, join, relative, resolve, sep } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, "..");
const currentBuild = "155";
const historicalBuilds = [
  "132", "133", "134", "135", "136", "137", "138", "139",
  "141", "142", "143", "144", "145", "146", "147", "148", "149",
  "150", "151", "152", "153", "154",
];
const excludedDirectories = new Set([
  ".git",
  "node_modules",
  "dist",
  "build",
  "target",
  "src-tauri/target",
  "releases",
  "artifacts",
  "coverage",
]);
const textExtensions = new Set([
  ".cjs",
  ".css",
  ".html",
  ".json",
  ".json5",
  ".js",
  ".jsx",
  ".md",
  ".mjs",
  ".ps1",
  ".rs",
  ".toml",
  ".ts",
  ".tsx",
  ".txt",
  ".yml",
  ".yaml",
]);

// Patterns for files that are intentional historical archives and must not be flagged
function isArchiveFile(rel) {
  const normalized = rel.split(sep).join("/");
  return (
    // Release notes for past builds
    /^RELEASE_NOTES_BUILD\d+\.md$/.test(normalized) ||
    // Build-specific test result logs
    /^build\d+-.*-test-results\.txt$/.test(normalized) ||
    /^build\d+-test-results\.txt$/.test(normalized) ||
    // Docs folder — historical audit documents
    normalized.startsWith("docs/") ||
    // Test files themselves reference historical build numbers in their names/assertions
    normalized.startsWith("tests/")
  );
}

function isExcludedDirectory(path) {
  const normalized = path.split(sep).join("/");
  return excludedDirectories.has(normalized) || normalized.startsWith("releases/") || normalized.startsWith("src-tauri/target/");
}

function walk(directory, files = []) {
  for (const entry of readdirSync(directory)) {
    const absolute = join(directory, entry);
    const rel = relative(root, absolute);
    const stats = statSync(absolute);

    if (stats.isDirectory()) {
      if (!isExcludedDirectory(rel)) walk(absolute, files);
      continue;
    }

    if (stats.isFile() && textExtensions.has(extname(entry))) {
      files.push(absolute);
    }
  }

  return files;
}

function stalePatternsFor(build) {
  return [
    new RegExp(`Build\\s*${build}`, "i"),
    new RegExp(`build[-_:]?${build}`, "i"),
    new RegExp(`B${build}\\b`, "i"),
    new RegExp(`1\\.0\\.${build}\\b`),
    new RegExp(`test:build${build}\\b`, "i"),
  ];
}

test("active repository files do not contain stale build-number drift", () => {
  const offenders = [];

  for (const file of walk(root)) {
    const rel = relative(root, file).split(sep).join("/");

    // Skip historical archive files — they legitimately reference old build numbers
    if (isArchiveFile(rel)) continue;

    const text = readFileSync(file, "utf8");

    for (const build of historicalBuilds) {
      for (const pattern of stalePatternsFor(build)) {
        if (pattern.test(text)) {
          offenders.push(`${rel} matched ${pattern}`);
        }
      }
    }
  }

  assert.deepEqual(offenders, [], `Stale build-number references found in active files while current build is ${currentBuild}.`);
});
