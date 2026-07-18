import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");

test("Casting has a dedicated full-page menu surface", () => {
  const launcher = read("src/components/CastButton.tsx");
  const page = read("src/components/tabs/CastingTab.tsx");
  assert.match(launcher, /Casting Center/);
  assert.match(launcher, /createPortal/);
  assert.match(page, /Nearby devices/);
  assert.match(page, /Refresh devices/);
});

test("Casting supports discovery without manual IP entry", () => {
  const page = read("src/components/tabs/CastingTab.tsx");
  const service = read("src/services/castDevices.ts");
  assert.doesNotMatch(page, /Device IP \/ Host/);
  assert.match(service, /discover_cast_devices/);
  assert.match(service, /cast_media_to_device/);
});

test("Google Cast, Samsung Smart View, and Apple AirPlay are represented", () => {
  const page = read("src/components/tabs/CastingTab.tsx");
  const backend = read("src-tauri/src/cast_devices.rs");
  for (const token of ["Google Cast", "Samsung Smart View", "Apple AirPlay"]) {
    assert.match(page, new RegExp(token));
  }
  assert.match(backend, /_googlecast\._tcp\.local\./);
  assert.match(backend, /_airplay\._tcp\.local\./);
  assert.match(backend, /MediaRenderer/);
});

test("Tauri registers casting discovery and playback commands", () => {
  const main = read("src-tauri/src/main.rs");
  assert.match(main, /mod cast_devices;/);
  assert.match(main, /cast_devices::discover_cast_devices/);
  assert.match(main, /cast_devices::cast_media_to_device/);
});
