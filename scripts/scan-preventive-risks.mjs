import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const build = JSON.parse(fs.readFileSync(path.join(root, "build-version.json"), "utf8"));
const findings = [];

function read(relativePath) {
  return fs.readFileSync(path.join(root, relativePath), "utf8");
}

function requireMarker(relativePath, marker, reason) {
  const content = read(relativePath);
  if (!content.includes(marker)) {
    findings.push({ severity: "high", file: relativePath, reason });
  }
}

function rejectPattern(relativePath, pattern, reason) {
  const content = read(relativePath);
  if (pattern.test(content)) {
    findings.push({ severity: "high", file: relativePath, reason });
  }
}

function rejectCrlf(relativePath) {
  const content = fs.readFileSync(path.join(root, relativePath));
  if (content.includes(Buffer.from("\r\n"))) {
    findings.push({
      severity: "medium",
      file: relativePath,
      reason: "CRLF line endings can recreate Windows-only exact-string test failures",
    });
  }
}

for (const required of [
  "schemaVersion",
  "productName",
  "semanticVersion",
  "displayBuild",
  "displayName",
  "releaseTag",
]) {
  assert(Object.hasOwn(build, required), `build-version.json missing ${required}`);
}
assert.equal(build.schemaVersion, 1);
assert.match(build.semanticVersion, /^\d+\.\d+\.\d+$/);
assert.match(build.displayName, /^v\d+ Build \d+(?:\.\d+)?$/);
assert.match(build.releaseTag, /^v\d+-build-\d+(?:\.\d+)?$/);

requireMarker(
  "src/buildInfo.ts",
  'import manifest from "../build-version.json"',
  "UI build identity must derive from build-version.json",
);
requireMarker(
  "src/main.tsx",
  "BUILD_INFO.displayName",
  "Startup diagnostics must use the authoritative build identity",
);
requireMarker(
  "src/components/Header.tsx",
  'import { BUILD_INFO } from "../buildInfo"',
  "Header build label must use the authoritative build identity",
);
requireMarker(
  "src/components/Sidebar.tsx",
  'import { BUILD_INFO } from "../buildInfo"',
  "Sidebar build label must use the authoritative build identity",
);
requireMarker(
  "src-tauri/src/build_identity.rs",
  'include_str!("../../build-version.json")',
  "Rust build identity must derive from build-version.json",
);
requireMarker(
  "src-tauri/src/main.rs",
  "build_identity::get_current_build_info()",
  "Rust app info must use the typed build identity",
);
requireMarker(
  ".github/workflows/release-build-170.yml",
  "npm run verify:master-release",
  "Packaging must be blocked by the master release gate",
);

const packageJson = JSON.parse(read("package.json"));
if (packageJson.version !== build.semanticVersion) {
  findings.push({
    severity: "high",
    file: "package.json",
    reason: `Package version ${packageJson.version} does not match ${build.semanticVersion}`,
  });
}
const cargoManifest = read("src-tauri/Cargo.toml");
if (!cargoManifest.includes(`version = "${build.semanticVersion}"`)) {
  findings.push({
    severity: "high",
    file: "src-tauri/Cargo.toml",
    reason: `Cargo package version does not match ${build.semanticVersion}`,
  });
}
const tauriConfiguration = JSON.parse(read("src-tauri/tauri.conf.json"));
if (tauriConfiguration.version !== build.semanticVersion) {
  findings.push({
    severity: "high",
    file: "src-tauri/tauri.conf.json",
    reason: `Tauri bundle version ${tauriConfiguration.version} does not match ${build.semanticVersion}`,
  });
}

const staleBuildPattern = /(?:Build 170|v2 Build 1\.0[0-3]|1\.7\.170)/;
for (const relativePath of [
  "src/components/Header.tsx",
  "src/components/Sidebar.tsx",
  "src/main.tsx",
  "src-tauri/src/main.rs",
  "src-tauri/src/embedded_server.rs",
]) {
  rejectPattern(
    relativePath,
    staleBuildPattern,
    `User-facing runtime code contains a stale build identity instead of ${build.displayName}`,
  );
}

for (const relativePath of [
  "build-version.json",
  "scripts/verify-master-build-gate.mjs",
  "scripts/verify-shared-contracts.mjs",
  "scripts/scan-preventive-risks.mjs",
  "contracts/v1/golden/metadata-provider-registry.json",
  "contracts/v1/golden/artwork-cache-entry.json",
]) {
  rejectCrlf(relativePath);
}

const commandPalette = read("src/components/Header.tsx");
if (/cv-command-palette[\s\S]{0,500}scale:\s*0\./.test(commandPalette)) {
  findings.push({
    severity: "high",
    file: "src/components/Header.tsx",
    reason: "Command palette reintroduced scale composition associated with WebView2 stalls",
  });
}

const providerRegistry = read("src-tauri/src/metadata_provider_config.rs");
if (!providerRegistry.includes("provider.enabled = true")) {
  findings.push({
    severity: "high",
    file: "src-tauri/src/metadata_provider_config.rs",
    reason: "Provider migration does not enforce the all-enabled policy",
  });
}
if (/api[_-]?key|access[_-]?token|client[_-]?secret/i.test(
  read("contracts/v1/golden/metadata-provider-registry.json"),
)) {
  findings.push({
    severity: "critical",
    file: "contracts/v1/golden/metadata-provider-registry.json",
    reason: "Portable provider contract appears to contain credentials",
  });
}

fs.mkdirSync(path.join(root, "master-evidence"), { recursive: true });
fs.writeFileSync(
  path.join(root, "master-evidence", "preventive-risk-findings.json"),
  `${JSON.stringify({ build: build.displayName, findings }, null, 2)}\n`,
);

if (findings.length > 0) {
  for (const finding of findings) {
    console.error(`[${finding.severity}] ${finding.file}: ${finding.reason}`);
  }
  process.exitCode = 1;
} else {
  console.log(`Preventive risk scan passed for ${build.displayName}.`);
}
