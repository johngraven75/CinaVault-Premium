import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const ui = readFileSync("src/components/tabs/SecurityTab.tsx", "utf8");
const vpn = readFileSync("src-tauri/src/vpn.rs", "utf8");
const profiles = readFileSync("src-tauri/src/vpn_profile_store.rs", "utf8");

test("WireGuard UI gives truthful persistent readiness and verified-default feedback", () => {
  assert.match(ui, /Official engine verified/);
  assert.match(ui, /Install official WireGuard/);
  assert.match(ui, /role="alert"/);
  assert.match(ui, /vpn_select_default/);
  assert.match(ui, /Save as verified default/);
  assert.doesNotMatch(ui, /profile\.path/);
});

test("WireGuard backend keeps startup bounded, cleanup explicit, and summaries secret-safe", () => {
  assert.match(vpn, /Duration::from_secs\(15\)/);
  assert.match(vpn, /cleanup\(\)\.await/);
  assert.match(vpn, /handshake_is_verified/);
  assert.match(profiles, /successful manual connection verification first/);
  const summary = profiles.slice(
    profiles.indexOf("pub struct StoredVpnProfile"),
    profiles.indexOf("struct ProfileState"),
  );
  assert.doesNotMatch(summary, /private_key|PrivateKey|path:/);
});
