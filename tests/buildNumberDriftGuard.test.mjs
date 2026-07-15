import test from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, extname, join, relative, resolve, sep } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, "..");
const currentBuild = "164";
const historicalBuilds = ["159", "160", "161", "162", "163"];
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
  ".cjs", ".css", ".html", ".json", ".json5", ".js", ".jsx",
  ".md", ".mjs", ".ps1", ".rs", ".toml", ".ts", ".tsx", ".txt",
  ".yml", ".yaml",
]);

function isArchiveFile(rel) {
  const normalized = rel.split(sep).join("/");
  return (
    /^RELEASE_NOTES_BUILD\d+\.md$/.test(normalized) ||
    /^build\d+-.*-test-results\.txt$/.test(normalized) ||
    /^build\d+-test-results\.txt$/.test(normalized) ||
    normalized.startsWith("docs/") ||
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

    if (stats.isFile() && textExtensions.has(extname(entry))) files.push(absolute);
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
    if (isArchiveFile(rel)) continue;

    const text = readFileSync(file, "utf8");
    for (const build of historicalBuilds) {
      for (const pattern of stalePatternsFor(build)) {
        if (pattern.test(text)) offenders.push(`${rel} matched ${pattern}`);
      }
    }
  }

  assert.deepEqual(offenders, [], `Stale build-number references found in active files while current build is ${currentBuild}.`);
});
