import test from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, extname, join, relative, resolve, sep } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, "..");
const currentBuild = "164";
const currentVersion = "1.6.4";
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

function readProjectFile(path) {
  return readFileSync(resolve(root, path), "utf8");
}

function readJson(path) {
  return JSON.parse(readProjectFile(path));
}

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

function cargoVersion(text) {
  const match = text.match(/^version\s*=\s*"([^"]+)"/m);
  assert.ok(match, "Cargo.toml package version was not found");
  return match[1];
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

test("version manifests carry forward together for the current build", () => {
  const packageJson = readJson("package.json");
  const packageLock = readJson("package-lock.json");
  const tauriConfig = readJson("src-tauri/tauri.conf.json");
  const cargoToml = readProjectFile("src-tauri/Cargo.toml");

  assert.equal(packageJson.version, currentVersion, "package.json version must match the current build version");
  assert.equal(packageLock.version, currentVersion, "package-lock.json root version must match the current build version");
  assert.equal(packageLock.packages[""].version, currentVersion, "package-lock.json package entry version must match the current build version");
  assert.equal(tauriConfig.version, currentVersion, "Tauri config version must match the current build version");
  assert.equal(cargoVersion(cargoToml), currentVersion, "Cargo.toml package version must match the current build version");
});

test("installer workflow carries forward the current build number", () => {
  const workflow = readProjectFile(".github/workflows/windows-installer.yml");

  assert.match(workflow, new RegExp(`BUILD_NUMBER:\\s*['\"]${currentBuild}['\"]`));
  assert.match(workflow, new RegExp(`BUILD_OUTPUT_DIR:\\s*releases/build-${currentBuild}`));
  assert.match(workflow, new RegExp(`CinaVault-Premium-Windows-Installer-Build${currentBuild}`));
  assert.equal(workflow.includes("releases/build-139"), false, "installer workflow must not publish to the old Build 139 release folder");
  assert.equal(workflow.includes("Build139"), false, "installer workflow must not publish the old Build 139 artifact name");
});
