import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");

test("space audio service provides click, menu, and transition effects", () => {
  const source = read("src/services/spaceAudio.ts");
  assert.match(source, /playSpaceClick/);
  assert.match(source, /playSpaceMenuSelect/);
  assert.match(source, /playSpaceTransition/);
  assert.match(source, /AudioContext/);
  assert.match(source, /createOscillator/);
});

test("startup announcement uses packaged audio with speech fallback", () => {
  const source = read("src/services/spaceAudio.ts");
  assert.match(source, /\/audio\/cinavault-startup\.mp3/);
  assert.match(source, /CinaVault Premier Server by Media Fire FL LLC/);
  assert.match(source, /SpeechSynthesisUtterance/);
  assert.match(source, /utterance\.pitch = 0\.72/);
  assert.match(source, /utterance\.rate = 0\.84/);
});

test("application initializes sounds and startup announcement at boot", () => {
  const entry = read("src/main.tsx");
  assert.match(entry, /initializeSpaceAudio\(\)/);
  assert.match(entry, /playStartupAnnouncement\(\)/);
});

test("global pointer handling covers buttons and menu transitions", () => {
  const source = read("src/services/spaceAudio.ts");
  assert.match(source, /document\.addEventListener\("pointerdown"/);
  assert.match(source, /button, \[role='button'\], a/);
  assert.match(source, /window\.setTimeout\(playSpaceTransition, 70\)/);
});
