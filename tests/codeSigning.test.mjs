import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const read = (file) => fs.readFileSync(file, "utf8");
const workflow = read(".github/workflows/windows-installer.yml");
const SIGNING_ENV = {
  AZURE_TENANT_ID: "tenant",
  AZURE_CLIENT_ID: "client",
  AZURE_CLIENT_SECRET: "secret",
  AZURE_ARTIFACT_SIGNING_ENDPOINT: "https://eus.codesigning.azure.net",
  AZURE_ARTIFACT_SIGNING_ACCOUNT: "cinavault",
  AZURE_ARTIFACT_SIGNING_CERTIFICATE_PROFILE: "public-trust",
};
const hasPwsh = spawnSync("pwsh", ["-NoProfile", "-Command", "exit 0"]).status === 0;

function runConfigure(extraEnv) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "cv-signing-"));
  const files = { env: path.join(dir, "env"), out: path.join(dir, "out"), config: path.join(dir, "signing.json") };
  fs.writeFileSync(files.env, "");
  fs.writeFileSync(files.out, "");
  const clean = Object.fromEntries(Object.entries(process.env).filter(([key]) => !(key in SIGNING_ENV)));
  const result = spawnSync(
    "pwsh",
    ["-NoProfile", "-File", "scripts/configure-windows-signing.ps1", "-ConfigPath", files.config],
    { encoding: "utf8", env: { ...clean, ...extraEnv, GITHUB_ENV: files.env, GITHUB_OUTPUT: files.out } },
  );
  return { result, files };
}

test("release build signs through signCommand only after signing is configured", () => {
  assert.match(workflow, /run: \.\/scripts\/configure-windows-signing\.ps1/);
  assert.match(workflow, /if: steps\.signing\.outputs\.enabled == 'true'\s+run: cargo install artifact-signing-cli --version 0\.11\.0 --locked/);
  assert.match(workflow, /npx --no-install tauri build --target x86_64-pc-windows-msvc @signing/);
  assert.doesNotMatch(workflow, /npm run tauri build -- /);
  assert.match(workflow, /\$signing = @\('--config', \$env:CINAVAULT_SIGNING_CONFIG\)/);
  assert.match(workflow, /run: \.\/scripts\/verify-windows-signatures\.ps1/);
});

test("signing secrets are read from secrets and never used in an if condition", () => {
  for (const name of ["AZURE_TENANT_ID", "AZURE_CLIENT_ID", "AZURE_CLIENT_SECRET"]) {
    assert.match(workflow, new RegExp(`${name}: \\$\\{\\{ secrets\\.${name} \\}\\}`));
  }
  assert.doesNotMatch(workflow, /if:[^\n]*secrets\./);
});

test("unconfigured signing leaves the build unsigned with a warning", { skip: !hasPwsh && "pwsh not installed" }, () => {
  const { result, files } = runConfigure({});
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /::warning title=Installers will be unsigned::/);
  assert.equal(read(files.out).trim(), "enabled=false");
  assert.equal(read(files.env).trim(), "");
  assert.equal(fs.existsSync(files.config), false);
});

test("configured signing writes a signCommand overlay that signs each file", { skip: !hasPwsh && "pwsh not installed" }, () => {
  const { result, files } = runConfigure(SIGNING_ENV);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(read(files.out).trim(), "enabled=true");
  assert.equal(read(files.env).trim(), `CINAVAULT_SIGNING_CONFIG=${files.config}`);
  const command = JSON.parse(read(files.config).replace(/^﻿/, "")).bundle.windows.signCommand;
  assert.equal(command.cmd, "pwsh");
  assert.equal(command.args.at(-1), "%1");
  assert.equal(command.args.at(-2), path.resolve("scripts/sign-windows.ps1"));
  assert.doesNotMatch(read(files.config), /secret/);
});

test("sign-windows refuses to run without the signing configuration", { skip: !hasPwsh && "pwsh not installed" }, () => {
  const clean = Object.fromEntries(Object.entries(process.env).filter(([key]) => !(key in SIGNING_ENV)));
  const result = spawnSync("pwsh", ["-NoProfile", "-File", "scripts/sign-windows.ps1", "package.json"], { encoding: "utf8", env: clean });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr + result.stdout, /Code signing is not configured; missing AZURE_TENANT_ID/);
});
