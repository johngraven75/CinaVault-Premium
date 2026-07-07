import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

test("Build 140 Google Cast service and clearly marked Cast UI are installed", () => {
  assert.equal(fs.existsSync("src/services/googleCast.ts"), true);
  assert.equal(fs.existsSync("src/components/CastButton.tsx"), true);

  const service = fs.readFileSync("src/services/googleCast.ts", "utf8");
  const ui = fs.readFileSync("src/components/CastButton.tsx", "utf8");

  assert.match(service, /castv2-client/);
  assert.match(service, /DefaultMediaReceiver/);
  assert.match(service, /castToGoogleDevice/);

  assert.match(ui, /📺 Cast|Cast/);
  assert.match(ui, /Google Cast/);
  assert.match(ui, /data-testid="cinavault-cast-button"/);
  assert.match(ui, /data-testid="cinavault-cast-tab"/);
});
