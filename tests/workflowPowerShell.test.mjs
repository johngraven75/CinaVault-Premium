import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

const hasPwsh = spawnSync("pwsh", ["-NoProfile", "-Command", "exit 0"]).status === 0;
const WORKFLOWS = [".github/workflows/v2-build-1-04-release.yml", ".github/workflows/windows-installer.yml"];

// Returns the `run: |` body of every step that declares `shell: pwsh`.
export function pwshRunBlocks(yamlText) {
  const lines = yamlText.split(/\r?\n/);
  const blocks = [];
  let stepStart = -1;
  for (let i = 0; i < lines.length; i += 1) {
    if (/^\s*- (name|uses|run|id):/.test(lines[i])) stepStart = i;
    const run = /^(\s*)run: \|\s*$/.exec(lines[i]);
    if (!run || stepStart < 0) continue;
    const header = lines.slice(stepStart, i).join("\n");
    if (!/^\s*shell: pwsh\s*$/m.test(header)) continue;
    const indent = run[1].length;
    const body = [];
    let j = i + 1;
    for (; j < lines.length; j += 1) {
      const line = lines[j];
      if (line.trim() !== "" && line.length - line.trimStart().length <= indent) break;
      body.push(line);
    }
    // YAML strips the block's common indentation; do the same so here-strings stay valid.
    const margin = Math.min(...body.filter((line) => line.trim()).map((line) => line.length - line.trimStart().length));
    const name = /- name: (.+)/.exec(header)?.[1] ?? `line ${i + 1}`;
    blocks.push({ name, script: body.map((line) => line.slice(margin)).join("\n").trimEnd() });
    i = j - 1;
  }
  return blocks;
}

test("pwshRunBlocks finds only PowerShell run blocks", () => {
  const yaml = [
    "steps:",
    "  - name: First",
    "    shell: pwsh",
    "    run: |",
    "      Write-Host one",
    "      Write-Host two",
    "  - name: Second",
    "    shell: bash",
    "    run: |",
    "      echo three",
  ].join("\n");
  assert.deepEqual(pwshRunBlocks(yaml), [{ name: "First", script: "Write-Host one\nWrite-Host two" }]);
});

test("release workflows contain the installer build step", () => {
  const names = pwshRunBlocks(fs.readFileSync(WORKFLOWS[0], "utf8")).map((block) => block.name);
  assert.ok(names.includes("Build Windows installers"), `found: ${names.join(", ")}`);
});

test("every PowerShell step in the release workflows parses", { skip: !hasPwsh && "pwsh not installed" }, () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "cv-pwsh-parse-"));
  const checker = path.join(dir, "parse.ps1");
  fs.writeFileSync(
    checker,
    [
      "param([string]$Path)",
      "$tokens = $null; $errors = $null",
      "[System.Management.Automation.Language.Parser]::ParseFile($Path, [ref]$tokens, [ref]$errors) | Out-Null",
      "$errors | ForEach-Object { \"line $($_.Extent.StartLineNumber): $($_.Message)\" }",
    ].join("\n"),
  );
  const failures = [];
  for (const workflow of WORKFLOWS) {
    for (const [index, block] of pwshRunBlocks(fs.readFileSync(workflow, "utf8")).entries()) {
      const file = path.join(dir, `step-${index}.ps1`);
      fs.writeFileSync(file, block.script);
      const result = spawnSync("pwsh", ["-NoProfile", "-File", checker, file], { encoding: "utf8" });
      const errors = result.stdout.trim();
      if (result.status !== 0 || errors) failures.push(`${workflow} › ${block.name}: ${errors || result.stderr}`);
    }
  }
  assert.deepEqual(failures, []);
});
