// Front-end rules for the media server, profile and parental-control switches.
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  PARENTAL_DEFAULTS,
  PROFILE_CHANGE_EVENTS,
  isHexColor,
  isPinRequired,
  parentalConfig,
  parseGenres,
  pinError,
  profileInitials,
  profileNameError,
  switchNeedsPin,
} from "../src/features/profileLogic.ts";
import {
  activityGroup,
  apiKeyStatus,
  effectiveMbps,
  mbpsHint,
  parseMbps,
  parseWebhookUrls,
  permissionLabels,
  relativeTime,
  webhookWants,
} from "../src/features/serverAdminLogic.ts";

const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
const kid = { id: 2, name: "Kid", color: "#f00", restricted: true, created_at: "" };
const kid2 = { ...kid, id: 3, name: "Kid 2" };
const owner = { id: 1, name: "Owner", color: "#38bdf8", restricted: false, created_at: "" };
const locked = { enabled: true, hasPin: true, unlocked: false, activeRestricted: true, ratedItems: 0, unratedItems: 0, maxRatingChoices: [] };

test("leaving a restricted profile asks for the PIN only when it is guarded", () => {
  assert.equal(switchNeedsPin(kid, owner, locked), true);
  assert.equal(switchNeedsPin(kid, kid2, locked), false, "restricted to restricted is free");
  assert.equal(switchNeedsPin(owner, kid, locked), false, "moving into a restricted profile is free");
  assert.equal(switchNeedsPin(kid, owner, { ...locked, unlocked: true }), false);
  assert.equal(switchNeedsPin(kid, owner, { ...locked, hasPin: false }), false);
  assert.equal(switchNeedsPin(kid, owner, { ...locked, enabled: false }), false);
  assert.equal(switchNeedsPin(kid, owner, null), false);
});

test("profile names, colours, initials and PINs follow the back-end rules", () => {
  assert.equal(profileNameError("  "), "A profile name needs 1 to 40 characters");
  assert.equal(profileNameError("x".repeat(41)) !== null, true);
  assert.equal(profileNameError("Movie Night"), null);
  assert.equal(isHexColor("#38BDF8"), true);
  assert.equal(isHexColor("#abc"), true);
  assert.equal(isHexColor("red"), false);
  assert.equal(profileInitials("movie night"), "MN");
  assert.equal(profileInitials("Ada"), "AD");
  assert.equal(profileInitials(""), "?");
  assert.equal(pinError("1234"), null);
  assert.equal(pinError("12a4"), "The PIN must be 4 to 8 digits");
  assert.equal(pinError("123456789"), "The PIN must be 4 to 8 digits");
  assert.equal(isPinRequired("PIN required: enter the parental PIN to leave a restricted profile"), true);
  assert.equal(isPinRequired("Wrong PIN"), false);
});

test("parental config parses genres and fills defaults", () => {
  assert.deepEqual(parseGenres("Horror, true crime ,,horror;Thriller\n"), ["Horror", "true crime", "Thriller"]);
  assert.deepEqual(parentalConfig(undefined), { ...PARENTAL_DEFAULTS, blockedGenres: [] });
  assert.equal(PARENTAL_DEFAULTS.maxRating, "PG", "matches DEFAULT_MAX_RATING in parental.rs");
  const merged = parentalConfig({ maxRating: "", blockedGenres: ["Horror", 3] });
  assert.equal(merged.maxRating, "");
  assert.deepEqual(merged.blockedGenres, ["Horror"]);
  assert.equal(merged.blockAdult, true);
});

test("profile changes reload the library and the discovery shelves", () => {
  assert.deepEqual([...PROFILE_CHANGE_EVENTS], ["cinavault:profile-changed", "cinavault:library-refresh"]);
  const service = read("src/services/profiles.ts");
  assert.match(service, /export async function switchProfile[\s\S]*announceLibraryChange/);
  const header = read("src/components/Header.tsx");
  assert.match(header, /<ProfileSwitcher \/>/);
  assert.match(read("src/components/profiles/ProfileSwitcher.tsx"), /useFeature\("user_profiles"\)/);
});

test("bandwidth cap parses, falls back to the old Remote Access setting, then 20", () => {
  assert.equal(parseMbps("12.34"), 12.3);
  assert.equal(parseMbps("0.1"), null);
  assert.equal(parseMbps("abc"), null);
  assert.equal(effectiveMbps({ mbps: 8 }, "50"), 8);
  assert.equal(effectiveMbps({}, "50"), 50);
  assert.equal(effectiveMbps(undefined, ""), 20);
  assert.equal(mbpsHint(30), "enough for 4K");
  assert.equal(mbpsHint(10), "enough for 1080p");
  assert.equal(mbpsHint(2), "standard definition");
});

test("Remote Access tab uses the matrix switches instead of its own settings", () => {
  const tab = read("src/components/tabs/RemoteAccessTab.tsx");
  assert.match(tab, /useFeature\("remote_access"\)/);
  assert.match(tab, /useFeature<\{ mbps\?: number \}>\("bandwidth_limit"\)/);
  assert.doesNotMatch(tab, /"remote_access_enabled"/);
  assert.doesNotMatch(read("src/store/appStore.ts"), /remote_access_enabled/);
});

test("webhook URLs and event filters match the back end", () => {
  assert.deepEqual(parseWebhookUrls("https://a.test/h\nftp://x\n\nhttp://lan/b, https://a.test/h"), {
    urls: ["https://a.test/h", "http://lan/b"],
    invalid: ["ftp://x"],
  });
  assert.equal(webhookWants([], "scan.finished"), true);
  assert.equal(webhookWants(["playback"], "playback.finished"), true);
  assert.equal(webhookWants(["playback"], "playbackish"), false);
  assert.equal(webhookWants(["server"], "scan.finished"), false);
});

test("API key and activity helpers format what the panels show", () => {
  assert.equal(apiKeyStatus({ id: 1, name: "a", prefix: "cvk_1", permissions: [], createdAt: "", revokedAt: "2026-01-01" }), "revoked");
  assert.equal(apiKeyStatus({ id: 1, name: "a", prefix: "cvk_1", permissions: [], createdAt: "" }), "active");
  assert.equal(permissionLabels(["library:read", "stream:play"]), "Read library, Stream");
  assert.equal(activityGroup("remote.login"), "remote");
  const now = new Date("2026-10-08T12:00:00Z");
  assert.equal(relativeTime("2026-10-08T11:59:50Z", now), "just now");
  assert.equal(relativeTime("2026-10-08T11:57:00Z", now), "3 min ago");
  assert.equal(relativeTime("2026-10-08T09:00:00Z", now), "3 h ago");
  assert.equal(relativeTime("2026-10-06T12:00:00Z", now), "2 d ago");
  assert.equal(relativeTime("not a date", now), "not a date");
});

test("every switch in this area has a settings panel", () => {
  for (const key of ["api_keys", "bandwidth_limit", "cdn_cache", "gpu_accel", "parental_ctrl", "user_profiles", "activity_log", "webhook"]) {
    assert.match(read(`src/features/panels/${key}.tsx`), /export default/);
  }
});
