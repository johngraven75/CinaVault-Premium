import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";

const read = (path) => fs.readFileSync(path, "utf8");

test("NAS sync keeps reqwest cookie support enabled", () => {
  const cargo = read("src-tauri/Cargo.toml");
  const reqwest = cargo.match(/^reqwest\s*=\s*\{[^}]*\}$/m);

  assert.ok(reqwest, "Cargo.toml must declare reqwest as an inline dependency");
  assert.match(reqwest[0], /\bcookies\b/);
});
