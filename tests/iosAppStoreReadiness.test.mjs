import test from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";

function read(path) {
  return readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
}

test("Tauri backend exposes a mobile library entry point for iOS", () => {
  assert.equal(existsSync(new URL("../src-tauri/src/lib.rs", import.meta.url)), true);

  const cargo = read("src-tauri/Cargo.toml");
  const main = read("src-tauri/src/main.rs");
  const lib = read("src-tauri/src/lib.rs");

  assert.match(cargo, /\[lib\]\s+crate-type = \["staticlib", "cdylib", "rlib"\]/);
  assert.match(lib, /#\[cfg_attr\(mobile,\s*tauri::mobile_entry_point\)\]\s+pub fn run\(\)/);
  assert.match(main, /cinavault_premium::run\(\);/);
});

test("iOS App Store build metadata uses the build-130 sequential bundle version", () => {
  const config = JSON.parse(read("src-tauri/tauri.conf.json"));

  assert.equal(config.identifier, "com.cinavault.premium");
  assert.equal(config.version, "1.0.0-18");
  assert.equal(config.bundle.iOS.bundleVersion, "130");
});

test("mobile build avoids native OpenSSL dependencies", () => {
  const cargo = read("src-tauri/Cargo.toml");

  assert.match(
    cargo,
    /reqwest = \{ version = "0\.12", default-features = false, features = \["json", "stream", "rustls-tls"\] \}/,
  );
});
