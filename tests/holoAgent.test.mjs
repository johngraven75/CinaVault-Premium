import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

import {
  AGENT_QUICK_PROMPTS,
  AGENT_START,
  FEATURE_POINT_COUNT,
  HEAD_CAMERA_DISTANCE,
  HEAD_REGION,
  JAW_DROP,
  AGENT_IMAGE_MAX_BYTES,
  AGENT_IMAGE_TYPES,
  activityLabel,
  agentReducer,
  base64FromDataUrl,
  buildHeadCloud,
  imageAttachmentError,
  mouthEnvelope,
  projectHeadPoint,
  summarizeAgentResult,
} from "../src/services/holoAgent.ts";

const read = (path) => fs.readFileSync(path, "utf8");
const run = (events, state = AGENT_START) => events.reduce(agentReducer, state);

test("agent moves idle → listening → thinking → speaking → idle", () => {
  let state = run([{ type: "focus" }]);
  assert.equal(state.mood, "listening");
  state = agentReducer(state, { type: "submit", prompt: "  Check all media sources " });
  assert.deepEqual(state, { mood: "thinking", pending: "Check all media sources" });
  state = agentReducer(state, { type: "answer" });
  assert.deepEqual(state, { mood: "speaking", pending: null });
  assert.equal(agentReducer(state, { type: "done" }).mood, "idle");
});

test("agent ignores empty prompts, double submits and stray events", () => {
  assert.equal(run([{ type: "submit", prompt: "   " }]), AGENT_START);
  const thinking = run([{ type: "submit", prompt: "first" }]);
  assert.equal(agentReducer(thinking, { type: "submit", prompt: "second" }).pending, "first");
  assert.equal(agentReducer(thinking, { type: "focus" }), thinking);
  assert.equal(run([{ type: "answer" }]), AGENT_START);
  assert.equal(run([{ type: "submit", prompt: "x" }, { type: "fail" }]).mood, "error");
  assert.equal(run([{ type: "submit", prompt: "x" }, { type: "fail" }, { type: "done" }]).mood, "idle");
});

test("head cloud has the requested points, both eyes, a mouth and a jaw", () => {
  const cloud = buildHeadCloud(1200, 40, 100);
  assert.equal(cloud.count, 1200 + 80 + 100 + FEATURE_POINT_COUNT);
  assert.equal(cloud.positions.length, cloud.count * 3);
  const regionCount = (region) => cloud.regions.filter((r) => r === region).length;
  assert.equal(regionCount(HEAD_REGION.eye), 80);
  assert.ok(regionCount(HEAD_REGION.mouth) > 0, "mouth points");
  assert.ok(regionCount(HEAD_REGION.jaw) > 0, "jaw points");
  for (const value of cloud.positions) assert.ok(Math.abs(value) <= 1.2, `point ${value} outside the head`);
  assert.deepEqual(buildHeadCloud(1200, 40, 100).positions, cloud.positions, "deterministic");
});

test("projection drops the jaw with the mouth and keeps the skull still", () => {
  const closed = projectHeadPoint(0, -0.6, 0.6, HEAD_REGION.jaw, { yaw: 0, mouth: 0 });
  const open = projectHeadPoint(0, -0.6, 0.6, HEAD_REGION.jaw, { yaw: 0, mouth: 1 });
  assert.ok(open.y < closed.y, "jaw moves down");
  const depth = HEAD_CAMERA_DISTANCE - 0.6;
  assert.equal(Number((closed.y - open.y).toFixed(6)), Number(((JAW_DROP * 2.4) / depth).toFixed(6)));
  const skullClosed = projectHeadPoint(0, 0.5, 0.2, HEAD_REGION.skull, { yaw: 0, mouth: 0 });
  const skullOpen = projectHeadPoint(0, 0.5, 0.2, HEAD_REGION.skull, { yaw: 0, mouth: 1 });
  assert.deepEqual(skullOpen, skullClosed);
  const turned = projectHeadPoint(0, 0, 1, HEAD_REGION.skull, { yaw: Math.PI / 2, mouth: 0 });
  assert.ok(turned.x > 0.5, `a quarter turn moves the face sideways (x=${turned.x})`);
  assert.equal(projectHeadPoint(0.5, 0, 0, 0, { yaw: 0, mouth: 0 }, 2).x * 2, projectHeadPoint(0.5, 0, 0, 0, { yaw: 0, mouth: 0 }, 1).x);
});

test("mouth envelope opens on vowels, closes on spaces and ends closed", () => {
  assert.deepEqual(mouthEnvelope("hi yo."), [0.35, 0.85, 0, 0.85, 0.85, 0, 0]);
  assert.equal(mouthEnvelope("a".repeat(1000), 60, 600).length, 11);
  assert.deepEqual(mouthEnvelope(""), [0]);
});

test("ai_query results become plain sentences", () => {
  assert.equal(summarizeAgentResult({ status: "success", message: "  Two sources   are offline. " }), "Two sources are offline.");
  assert.equal(summarizeAgentResult({ status: "error", message: "No HF token" }), "That didn't work: No HF token");
  assert.equal(summarizeAgentResult({ status: "error" }), "That didn't work.");
  assert.equal(
    summarizeAgentResult({
      dns: { test: "DNS Resolution", success: true },
      ping: { test: "Ping (Google DNS)", success: false },
      http: { test: "HTTPS Connectivity", success: true },
    }),
    "2 of 3 checks passed. Failed: Ping (Google DNS).",
  );
  assert.equal(summarizeAgentResult({ a: { test: "A", success: true } }), "All 1 checks passed.");
  assert.equal(summarizeAgentResult({ sources_checked: 4, offline: 1, label: "x" }), "Finished: sources checked 4, offline 1.");
  assert.equal(summarizeAgentResult([1, 2, 3]), "Found 3 results.");
  assert.equal(summarizeAgentResult(null), "Done.");
  assert.equal(summarizeAgentResult("x".repeat(700)).length, 600);
});

test("quick prompts are read-only checks the back end routes away from library changes", () => {
  for (const prompt of AGENT_QUICK_PROMPTS) {
    assert.doesNotMatch(prompt.toLowerCase(), /enrich|rename|clean|duplicate|poster|tag|normalize|nfo/);
  }
});

test("agent is mounted behind its Advanced switch and lazy-loaded", () => {
  const app = read("src/App.tsx");
  assert.match(app, /const HoloAgent = lazy\(\(\) => import\("\.\/components\/agent\/HoloAgent"\)\)/);
  assert.match(app, /isFeatureOn\(featureSettings, "holo_agent"\) && \(/);
  assert.match(read("src/components/agent/HoloAgent.tsx"), /invoke<unknown>\("ai_query", \{ prompt: clean \}\)/);
  assert.match(read("src-tauri/src/lib.rs"), /ai::ai_query/);
  const head = read("src/components/agent/HoloHead.tsx");
  assert.match(head, /if \(reduceMotion\) draw\(0\);\s*else frame = requestAnimationFrame\(loop\);/);
  assert.match(head, /canvas\.getContext\("2d"\)/, "2D fallback when WebGL2 is unavailable");
});

test("activity from the Claude brain becomes a readable status line", () => {
  assert.equal(activityLabel("thinking", null), "Thinking");
  assert.equal(activityLabel("searching", "search_library"), "Searching your library");
  assert.equal(activityLabel("searching", "view_poster"), "Looking at the poster");
  assert.equal(activityLabel("searching", "run_diagnostics"), "Running diagnostics");
  assert.equal(activityLabel("searching", "something_new"), "Checking");
  assert.equal(activityLabel("acting", "play_media"), "Preparing an action for you");
  assert.equal(activityLabel("idle", null), "Ready");
});

test("image attachments are checked before they are encoded", () => {
  assert.deepEqual([...AGENT_IMAGE_TYPES], ["image/jpeg", "image/png", "image/gif", "image/webp"]);
  assert.equal(imageAttachmentError("image/png", 120_000), null);
  assert.match(imageAttachmentError("image/svg+xml", 1000), /JPEG, PNG, GIF or WebP/);
  assert.match(imageAttachmentError("image/jpeg", 0), /empty/);
  // The encoded size is what the API limits, so a 4.5 MB file is too big.
  assert.ok(imageAttachmentError("image/jpeg", 4.5 * 1024 * 1024));
  assert.equal(imageAttachmentError("image/jpeg", Math.floor((AGENT_IMAGE_MAX_BYTES / 4) * 3) - 3), null);
  assert.equal(base64FromDataUrl("data:image/png;base64,iVBORw0KGgo="), "iVBORw0KGgo=");
  assert.equal(base64FromDataUrl("iVBORw0KGgo="), "iVBORw0KGgo=");
});

test("the Claude brain is wired front to back and keeps the key in Rust", () => {
  const lib = read("src-tauri/src/lib.rs");
  const rust = read("src-tauri/src/ai_agent.rs");
  const brain = read("src/services/agentBrain.ts");
  const panel = read("src/components/agent/HoloAgent.tsx");
  for (const command of [
    "agent_status",
    "agent_set_api_key",
    "agent_clear_api_key",
    "agent_set_model",
    "agent_reset",
    "agent_chat",
    "agent_run_action",
  ]) {
    assert.match(lib, new RegExp(`ai_agent::${command},`), `${command} is registered`);
    assert.match(brain, new RegExp(`"${command}"`), `${command} is called from the front end`);
  }
  // Every action kind Rust can propose has a matching TypeScript shape.
  for (const kind of ["play", "refreshMetadata", "rename", "setWatched", "discoverFolders", "organizeLibrary"]) {
    assert.match(brain, new RegExp(`type: "${kind}"`), kind);
  }
  assert.match(rust, /DiscoverFolders,\s*OrganizeLibrary \{ tasks: Vec<String> \},/);
  // The key never travels back to the WebView, and no SDK ships in the bundle.
  assert.doesNotMatch(rust, /pub key:|api_key: String,\s*\n\s*pub/);
  assert.match(rust, /pub key_source: Option<&'static str>/);
  assert.doesNotMatch(brain + panel, /@anthropic-ai\/sdk/);
  assert.match(panel, /type="password"/);
  // Without a key the panel keeps working offline through ai_query.
  assert.match(panel, /useClaude \? attachment : null/);
  assert.match(panel, /sendAgentMessage\(clean, image\?\.input \?\? null/);
  // Anything that changes the library is a button the user presses.
  assert.match(panel, /runAgentAction\(action\.id\)/);
});
