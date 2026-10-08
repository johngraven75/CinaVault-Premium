import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  beaconLabel,
  computeHoloPointer,
  DEFAULT_MAX_TILT_DEG,
  HOLO_REST,
  holoPointerStyleVars,
  progressFraction,
  staggerDelay,
} from "../src/utils/holoMotion.ts";

const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
const rect = { left: 100, top: 200, width: 200, height: 300 };

test("pointer at the card centre leaves the card at rest", () => {
  assert.deepEqual(computeHoloPointer(200, 350, rect), { ...HOLO_REST });
});

test("pointer at the top-right corner tilts back and right at full intensity", () => {
  const state = computeHoloPointer(300, 200, rect);
  assert.equal(state.rotateX, DEFAULT_MAX_TILT_DEG);
  assert.equal(state.rotateY, DEFAULT_MAX_TILT_DEG);
  assert.equal(state.px, 1);
  assert.equal(state.py, 0);
  assert.equal(state.intensity, 1);
});

test("pointer at the bottom-left corner tilts forward and left", () => {
  const state = computeHoloPointer(100, 500, rect, 8);
  assert.equal(state.rotateX, -8);
  assert.equal(state.rotateY, -8);
});

test("points outside the card clamp to its edge and never exceed the max tilt", () => {
  const state = computeHoloPointer(10_000, -10_000, rect, 10);
  assert.equal(state.px, 1);
  assert.equal(state.py, 0);
  assert.ok(Math.abs(state.rotateX) <= 10 && Math.abs(state.rotateY) <= 10);
});

test("a zero-sized or non-finite card rect returns the rest state", () => {
  assert.deepEqual(computeHoloPointer(5, 5, { left: 0, top: 0, width: 0, height: 10 }), { ...HOLO_REST });
  assert.deepEqual(computeHoloPointer(5, 5, { left: 0, top: 0, width: Number.NaN, height: 10 }), { ...HOLO_REST });
  const nan = computeHoloPointer(Number.NaN, 350, rect);
  assert.equal(nan.px, 0.5);
});

test("style vars carry units the stylesheet expects", () => {
  const vars = holoPointerStyleVars(computeHoloPointer(300, 200, rect, 10));
  assert.equal(vars["--holo-rx"], "10deg");
  assert.equal(vars["--holo-ry"], "10deg");
  assert.equal(vars["--holo-px"], "1");
  assert.equal(vars["--holo-py"], "0");
  assert.equal(vars["--holo-intensity"], "1");
});

test("entrance stagger grows per card and is capped for long libraries", () => {
  assert.equal(staggerDelay(0), 0);
  assert.equal(staggerDelay(2), 0.07);
  assert.equal(staggerDelay(5000), 0.42);
  assert.equal(staggerDelay(-3), 0);
  assert.equal(staggerDelay(Number.NaN), 0);
});

test("progress fraction is clamped and unknown totals stay indeterminate", () => {
  assert.equal(progressFraction(50, 200), 0.25);
  assert.equal(progressFraction(300, 200), 1);
  assert.equal(progressFraction(-1, 200), 0);
  assert.equal(progressFraction(10, 0), null);
  assert.equal(progressFraction(10, null), null);
  assert.equal(progressFraction(undefined, 10), null);
});

test("beacon labels cover every status", () => {
  assert.equal(beaconLabel("online"), "Online");
  assert.equal(beaconLabel("syncing"), "Syncing");
  assert.equal(beaconLabel("warning"), "Needs attention");
  assert.equal(beaconLabel("offline"), "Offline");
});

test("holographic styles animate only compositor-friendly properties and honour reduced motion", () => {
  const css = read("src/styles/holo-cinema.css");
  assert.match(css, /@media \(prefers-reduced-motion: reduce\)/);
  const keyframes = css.match(/@keyframes [\s\S]*?\n}\n/g) ?? [];
  assert.ok(keyframes.length >= 8, "expected the holo keyframes");
  for (const block of keyframes) {
    const props = [...block.matchAll(/^\s*([a-z-]+):/gm)].map((match) => match[1]);
    for (const prop of props) {
      assert.ok(["transform", "opacity"].includes(prop), `${prop} animated in ${block.split("\n")[0]}`);
    }
  }
  for (const transition of css.matchAll(/transition:\s*([^;]+);/g)) {
    for (const part of transition[1].split(",")) {
      const prop = part.trim().split(/\s+/)[0];
      assert.ok(
        ["transform", "opacity", "background-color", "border-color", "none"].includes(prop),
        `transition on ${prop}`,
      );
    }
  }
  assert.match(read("src/main.tsx"), /import "\.\/styles\/holo-cinema\.css";/);
});

test("app shell respects the OS motion preference and tab transitions avoid blur", () => {
  const app = read("src/App.tsx");
  // The OS preference applies unless Settings > Visual Effects turns motion off entirely.
  assert.match(app, /<MotionConfig reducedMotion=\{shell.reduceMotion ? "always" : "user"\}>/);
  const tabMotion = app.slice(app.indexOf("const TAB_MOTION"), app.indexOf("function findScrollableAncestor"));
  assert.doesNotMatch(tabMotion, /filter/);
  for (const effect of ["MeteorShower", "ParticleField", "AIVisualizer"]) {
    assert.match(
      read(`src/components/effects/${effect}.tsx`),
      /if \(!reduceMotion\) animId = requestAnimationFrame\(draw\);/,
      `${effect} must stop looping under reduced motion`,
    );
  }
});

test("library cards are holographic and tracking stays off the React render path", () => {
  const home = read("src/components/tabs/HomeTab.tsx");
  assert.match(home, /<HoloCard/);
  assert.match(home, /className="holo-grid"/);
  assert.match(home, /<HoloSkeletonCard/);
  const card = read("src/components/holo/HoloCard.tsx");
  assert.match(card, /useReducedMotion/);
  assert.match(card, /requestAnimationFrame\(flush\)/);
  assert.match(card, /pointerType === "touch"/);
  assert.doesNotMatch(card, /useState/);
});
