import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

import {
  PORTRAIT_WEIGHTS,
  blinkAt,
  buildPortraitMesh,
  calibrationError,
  defaultCalibration,
  deformPortraitPoint,
  faceFrame,
  portraitImageError,
  portraitPose,
  portraitView,
  portraitWeights,
} from "../src/services/portraitAvatar.ts";

const read = (path) => fs.readFileSync(path, "utf8").replace(/\r/g, "");
const cal = defaultCalibration();
const NEUTRAL = { yaw: 0, pitch: 0, roll: 0, blink: 0, brow: 0, mouth: 0 };
// A tall full-body photo, like a typical phone portrait.
const tall = { leftEye: { x: 0.425, y: 0.252 }, rightEye: { x: 0.575, y: 0.252 }, mouth: { x: 0.5, y: 0.345 } };
const TALL_ASPECT = 1000 / 1500;

test("default markers and a tall photo's markers are accepted", () => {
  assert.equal(calibrationError(cal), null);
  assert.equal(calibrationError(tall, TALL_ASPECT), null);
});

test("marker distances are judged in the photo's real proportions", () => {
  // Fine on a square photo, but on a tall one the mouth is far below the eyes.
  const lowMouth = { ...tall, mouth: { x: 0.5, y: 0.6 } };
  assert.equal(calibrationError(lowMouth, 1), null);
  assert.match(calibrationError(lowMouth, TALL_ASPECT), /too far/);
});

test("misplaced markers are explained", () => {
  const with_ = (patch) => calibrationError({ ...cal, ...patch });
  assert.match(with_({ leftEye: { x: 0.7, y: 0.4 } }), /left of/);
  assert.match(with_({ leftEye: { x: 0.49, y: 0.4 }, rightEye: { x: 0.51, y: 0.4 } }), /apart/);
  assert.match(with_({ mouth: { x: 0.5, y: 0.42 } }), /down onto/);
  assert.match(with_({ leftEye: { x: 0.4, y: 0.1 }, rightEye: { x: 0.6, y: 0.1 }, mouth: { x: 0.5, y: 0.95 } }), /too far/);
  assert.match(with_({ mouth: { x: 1.4, y: 0.6 } }), /on the photo/);
  assert.match(with_({ mouth: { x: Number.NaN, y: 0.6 } }), /on the photo/);
});

test("only JPEG, PNG and WebP photos under 12 MB are accepted", () => {
  assert.equal(portraitImageError("image/jpeg", 2_000_000), null);
  assert.equal(portraitImageError("image/webp", 1), null);
  assert.match(portraitImageError("image/gif", 1000), /JPEG, PNG or WebP/);
  assert.match(portraitImageError("image/png", 0), /empty/);
  assert.match(portraitImageError("image/png", 13 * 1024 * 1024), /12 MB/);
});

test("the face frame follows a tilted head", () => {
  const tilted = { leftEye: { x: 0.4, y: 0.38 }, rightEye: { x: 0.6, y: 0.42 }, mouth: { x: 0.49, y: 0.6 } };
  const frame = faceFrame(tilted);
  assert.ok(frame.down.x < 0 && frame.down.y > 0.9, "down leans with the tilt");
  assert.ok(Math.abs(frame.eyeDistance - Math.hypot(0.2, 0.04)) < 1e-9);
});

test("weights: the jaw moves below the mouth, lids sit on the eyes, the background stays put", () => {
  const [depthMid, jawMid, lidMid] = portraitWeights(cal, 0.5, 0.4);
  assert.ok(depthMid > 0.5, "the face bulges forward");
  assert.equal(Number(jawMid.toFixed(6)), 0);
  assert.ok(lidMid < 0.02, `no eyelid between the eyes (${lidMid})`);
  const chin = portraitWeights(cal, 0.5, 0.68);
  assert.ok(chin[1] > 0.6, `chin follows the jaw (${chin[1]})`);
  const lid = portraitWeights(cal, 0.4, 0.39);
  assert.ok(lid[2] > 0.5, `upper lid over the left eye (${lid[2]})`);
  assert.ok(portraitWeights(cal, 0.6, 0.35)[3] > 0.5, "brow above the right eye");
  assert.ok(portraitWeights(cal, 0.5, 0.6)[4] > 0.9, "lip gap at the mouth");
  const corner = portraitWeights(cal, 0.02, 0.02);
  assert.deepEqual(corner.slice(0, 5).map((w) => Number(w.toFixed(4))), [0, 0, 0, 0, 0]);
  assert.ok(corner[5] < 0.01, "the corner is outside the head");
  assert.ok(portraitWeights(cal, 0.5, 0.25)[5] > 0.9, "the forehead is inside the head");
});

test("deformation: neutral pose is the identity; mouth, blink, brow and turn move the right parts", () => {
  const close = (a, b) => Math.abs(a.x - b.x) < 1e-9 && Math.abs(a.y - b.y) < 1e-9;
  for (const [u, v] of [[0.1, 0.1], [0.5, 0.5], [0.42, 0.39]]) {
    assert.ok(close(deformPortraitPoint(cal, u, v, NEUTRAL), { x: u, y: v }));
  }
  const chin = deformPortraitPoint(cal, 0.5, 0.68, { ...NEUTRAL, mouth: 1 });
  assert.ok(chin.y > 0.68 + 0.01, "the chin drops when the mouth opens");
  const eye = deformPortraitPoint(cal, 0.4, 0.4, { ...NEUTRAL, mouth: 1 });
  assert.ok(Math.abs(eye.y - 0.4) < 1e-6, "the eyes stay put while speaking");
  assert.ok(deformPortraitPoint(cal, 0.4, 0.39, { ...NEUTRAL, blink: 1 }).y > 0.39 + 0.005, "the lid closes downward");
  assert.ok(deformPortraitPoint(cal, 0.6, 0.35, { ...NEUTRAL, brow: 1 }).y < 0.35, "brows lift");
  const turn = { ...NEUTRAL, yaw: 0.1 };
  const noseShift = deformPortraitPoint(cal, 0.5, 0.5, turn).x - 0.5;
  const cornerShift = deformPortraitPoint(cal, 0.02, 0.02, turn).x - 0.02;
  assert.ok(noseShift > 0.005 && Math.abs(cornerShift) < 1e-9, "parallax moves the face, not the background");
});

test("blinks are brief, bounded and regular", () => {
  let closed = 0;
  let max = 0;
  const samples = 2000;
  for (let i = 0; i < samples; i += 1) {
    const value = blinkAt(i * 0.01);
    assert.ok(value >= 0 && value <= 1);
    if (value > 0.5) closed += 1;
    max = Math.max(max, value);
  }
  assert.ok(max > 0.9, "the eyes do close");
  assert.ok(closed / samples < 0.08, `mostly open (${closed}/${samples})`);
  assert.equal(blinkAt(12.34), blinkAt(12.34), "deterministic");
});

test("each mood has its own pose and reduced motion holds still", () => {
  const at = 2.2;
  assert.ok(portraitPose("listening", at).brow > 0.4, "listening raises the brows");
  assert.ok(portraitPose("thinking", at).blink >= 0.3 && portraitPose("thinking", at).pitch < 0, "thinking narrows the eyes and looks up");
  assert.equal(portraitPose("speaking", at, 0.8).mouth, 0.8);
  assert.equal(portraitPose("speaking", at, 3).mouth, 1);
  assert.equal(portraitPose("idle", at, 1).mouth, 0, "only speaking opens the mouth");
  assert.ok(portraitPose("error", at).brow < 0, "errors frown");
  const still = portraitPose("thinking", at, 1, true);
  assert.deepEqual({ ...still, brow: 0 }, NEUTRAL);
});

test("the mesh covers the whole photo with valid triangles", () => {
  const mesh = buildPortraitMesh(cal, 8, 6);
  assert.equal(mesh.vertexCount, 9 * 7);
  assert.equal(mesh.uvs.length, mesh.vertexCount * 2);
  assert.equal(mesh.weights.length, mesh.vertexCount * PORTRAIT_WEIGHTS);
  assert.equal(mesh.indices.length, 8 * 6 * 6);
  assert.ok(mesh.indices.every((i) => i < mesh.vertexCount));
  assert.deepEqual([mesh.uvs.at(-2), mesh.uvs.at(-1)], [1, 1]);
});

test("the view frames the face even in a full-body photo", () => {
  const view = portraitView(tall, TALL_ASPECT);
  assert.ok(view.center.y > 0.252 && view.center.y < 0.345, `centred between eyes and mouth (${view.center.y})`);
  assert.ok(Math.abs(view.center.x - 0.5) < 1e-9);
  assert.ok(view.height > 0.3 && view.height < 0.6, `a close-up, not the whole photo (${view.height})`);
});

test("the photo comes from app data, never from the bundle", () => {
  const head = read("src/components/agent/PortraitHead.tsx");
  const setup = read("src/components/agent/PortraitSetup.tsx");
  for (const source of [head, setup]) {
    assert.doesNotMatch(source, /from\s+["'][^"']+\.(png|jpe?g|webp)["']/, "no bundled photo");
  }
  const panel = read("src/components/agent/HoloAgent.tsx");
  assert.match(panel, /getAgentAvatar\(\)/);
  assert.match(panel, /avatar \? \(\s*<PortraitHead/);
  assert.match(panel, /<HoloHead /, "the hologram head stays as the fallback");
  const lib = read("src-tauri/src/lib.rs");
  for (const command of ["agent_avatar_get", "agent_avatar_save", "agent_avatar_set_calibration", "agent_avatar_clear"]) {
    assert.match(lib, new RegExp(`agent_avatar::${command},`));
  }
  const rust = read("src-tauri/src/agent_avatar.rs");
  assert.match(rust, /app_data_dir\(\)/);
  assert.match(rust, /JpegEncoder/, "re-encoded, which drops EXIF location data");
});
