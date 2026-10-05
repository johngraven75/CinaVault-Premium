import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

// The floating CastButton was replaced by a dedicated Casting tab and a
// header shortcut; Chromecast delivery now runs natively through rust_cast.
test("Google Cast service and clearly marked Cast UI remain installed", () => {
  assert.equal(fs.existsSync("src/services/castingService.ts"), true);
  assert.equal(fs.existsSync("src/components/tabs/CastingTab.tsx"), true);

  const service = fs.readFileSync("src/services/castingService.ts", "utf8");
  const tab = fs.readFileSync("src/components/tabs/CastingTab.tsx", "utf8");
  const header = fs.readFileSync("src/components/Header.tsx", "utf8");
  const native = fs.readFileSync("src-tauri/src/casting.rs", "utf8");

  assert.match(service, /discover_casting_devices/);
  assert.match(service, /start_casting/);
  assert.match(service, /CastingDeviceType = "chromecast"/);

  assert.match(tab, /data-testid="cinavault-casting-tab"/);
  assert.match(tab, /Chromecast/);
  assert.match(header, /Open Casting Center/);
  assert.match(native, /fn cast_chromecast_media/);
});
