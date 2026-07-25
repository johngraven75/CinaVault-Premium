import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

function read(relativePath) {
  const absolutePath = resolve(ROOT, relativePath);
  assert.ok(existsSync(absolutePath), `Required Build 170 file is missing: ${relativePath}`);
  return readFileSync(absolutePath, "utf8");
}

test("Build 170 native connectivity controller includes direct and relay paths", () => {
  const source = read("src-tauri/src/remote_connectivity.rs");
  for (const token of [
    "start_remote_connectivity",
    "stop_remote_connectivity",
    "get_remote_connectivity_status",
    "map_upnp",
    "map_nat_pmp",
    "PortMappingProtocol::TCP",
    "Protocol::TCP",
    "CINAVAULT_CLOUDFLARE_TUNNEL_TOKEN",
    "CINAVAULT_CLOUDFLARE_PUBLIC_URL",
    ".trycloudflare.com",
    "MAPPING_RENEW_SECONDS",
  ]) {
    assert.ok(source.includes(token), `Build 170 native token missing: ${token}`);
  }
});

test("Build 170 Tauri startup and command registry are wired", () => {
  const main = read("src-tauri/src/main.rs");
  for (const token of [
    "mod remote_connectivity;",
    "remote_connectivity::configure",
    "remote_connectivity::start_remote_connectivity",
    "remote_connectivity::stop_remote_connectivity",
    "remote_connectivity::get_remote_connectivity_status",
    '"build": "170"',
    '"automaticNatTraversal": true',
    '"cloudRelayFallback": true',
  ]) {
    assert.ok(main.includes(token), `Build 170 main wiring missing: ${token}`);
  }
});

test("Build 170 remote access UI controls and displays live connectivity", () => {
  const ui = read("src/components/tabs/RemoteAccessTab.tsx");
  for (const token of [
    "RemoteConnectivityStatus",
    '"get_remote_connectivity_status"',
    '"start_remote_connectivity"',
    '"stop_remote_connectivity"',
    "directAvailable",
    "relayActive",
    "preferredUrl",
    "Automatic NAT Traversal",
    "Automatic Relay Fallback",
  ]) {
    assert.ok(ui.includes(token), `Build 170 UI token missing: ${token}`);
  }
});

test("Build 170 packaging includes the relay client and release version", () => {
  const config = JSON.parse(read("src-tauri/tauri.conf.json"));
  assert.equal(config.version, "1.7.170");
  assert.ok(
    config.bundle.resources.includes("tools/cloudflared/*"),
    "cloudflared bundle resource is not configured",
  );
  read("src-tauri/tools/cloudflared/README.txt");
});

test("Build 170 Cargo dependencies include both NAT traversal protocols", () => {
  const cargo = read("src-tauri/Cargo.toml");
  assert.ok(cargo.includes('igd-next = "0.17.1"'));
  assert.ok(cargo.includes('natpmp = { version = "0.5", features = ["tokio"] }'));
});
