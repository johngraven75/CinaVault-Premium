import test from "node:test";
import assert from "node:assert/strict";

const { shellPreferences, sanitizeCustomCss } = await import("../src/features/shellPreferences.ts");

const on = (enabled, config = {}) => ({ enabled, config });

test("look switches become document classes", () => {
  const prefs = shellPreferences({}, { glass_effects: on(false), compact_mode: on(true), poster_hover: on(false) });
  assert.deepEqual(prefs.classes, {
    "cv-no-glass": true,
    "cv-compact": true,
    "cv-no-poster-hover": true,
    "cv-reduce-motion": false,
  });
  const defaults = shellPreferences({}, {});
  assert.deepEqual(defaults.classes, {
    "cv-no-glass": false,
    "cv-compact": false,
    "cv-no-poster-hover": false,
    "cv-reduce-motion": false,
  });
});

test("motion and window opacity come from Settings", () => {
  const prefs = shellPreferences({ motion_enabled: "false", window_opacity: "40" }, {});
  assert.equal(prefs.reduceMotion, true);
  assert.equal(prefs.classes["cv-reduce-motion"], true);
  assert.equal(prefs.backdropOpacity, 0.6, "opacity is clamped to the slider minimum");
  assert.equal(shellPreferences({ window_opacity: "85" }, {}).backdropOpacity, 0.85);
});

test("custom CSS applies only while its switch is on, without remote loads", () => {
  const css = "body{color:red}@import url(https://evil.test/x.css);.a{background:url(https://evil.test/p.png)}</style><script>";
  assert.equal(shellPreferences({}, { custom_css: on(false, { css }) }).customCss, "");
  const applied = shellPreferences({}, { custom_css: on(true, { css }) }).customCss;
  assert.equal(applied, "body{color:red}.a{background:none}><script>");
  assert.equal(sanitizeCustomCss(42), "");
  assert.equal(sanitizeCustomCss(".a{background:url( \"https://evil.test/p.png\" )}"), ".a{background:none}");
  assert.doesNotMatch(sanitizeCustomCss(".a{background:url('javascript:alert(1)')}"), /javascript:/);
});
