import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  PLUS_FEATURES,
  PLUS_PRICE_LABEL,
  handlePaywallError,
  isFeatureUnlocked,
  normalizeEntitlements,
  normalizePlusFeature,
  parsePaywallError,
  paywallAwareErrorMessage,
  paywallShownByGate,
  planLabel,
  plusFeatureLabel,
  useEntitlementsStore,
  visiblePlusFeatures,
} from "../src/services/entitlements.ts";

test("parsePaywallError extracts feature and message from PAYWALL refusals", () => {
  assert.deepEqual(
    parsePaywallError("PAYWALL:downloads:Downloads require CinaVault Plus"),
    { feature: "downloads", message: "Downloads require CinaVault Plus" },
  );
  assert.deepEqual(
    parsePaywallError(new Error("PAYWALL:remote_access: Remote access needs Plus")),
    { feature: "remote_access", message: "Remote access needs Plus" },
  );
  assert.deepEqual(
    parsePaywallError({ message: "PAYWALL:external_libraries:NAS is a Plus feature: upgrade" }),
    { feature: "external_libraries", message: "NAS is a Plus feature: upgrade" },
  );
  const bare = parsePaywallError("PAYWALL:adult_metadata:");
  assert.equal(bare.feature, "adult_metadata");
  assert.match(bare.message, /CinaVault Plus/);
});

test("parsePaywallError ignores ordinary errors and unknown features", () => {
  assert.equal(parsePaywallError("Network unreachable"), null);
  assert.equal(parsePaywallError(new Error("paywall:downloads:lowercase prefix")), null);
  assert.equal(parsePaywallError("PAYWALL:teleportation:nope"), null);
  assert.equal(parsePaywallError(null), null);
  assert.equal(parsePaywallError(undefined), null);
  assert.equal(parsePaywallError(42), null);
});

test("feature mapping covers the four Plus features and tolerates aliases", () => {
  assert.deepEqual(
    PLUS_FEATURES.map((feature) => feature.id),
    ["external_libraries", "downloads", "adult_metadata", "remote_access"],
  );
  assert.equal(normalizePlusFeature("NAS"), "external_libraries");
  assert.equal(normalizePlusFeature("remote-access"), "remote_access");
  assert.equal(normalizePlusFeature("Download"), "downloads");
  assert.equal(normalizePlusFeature("nothing"), null);
  assert.equal(plusFeatureLabel("downloads"), "Downloads");
  assert.equal(plusFeatureLabel("mystery"), "mystery");
  assert.deepEqual(
    visiblePlusFeatures(true).map((feature) => feature.id),
    ["external_libraries", "downloads", "remote_access"],
  );
  assert.equal(visiblePlusFeatures(false).length, 4);
  assert.equal(PLUS_PRICE_LABEL, "$9.99/month");
});

test("entitlement snapshots gate features and fail open when unknown", () => {
  const free = normalizeEntitlements({
    plan: "free",
    features: { external_libraries: false, downloads: false, adult_metadata: false, remote_access: false },
    checkout_url: "",
  });
  assert.equal(isFeatureUnlocked(free, "downloads"), false);
  assert.equal(free.checkout_url, null);
  assert.equal(free.price_label, "$9.99/month");
  assert.equal(planLabel(free), "CinaVault Free");

  const trial = normalizeEntitlements({
    plan: "trial",
    trial_days_left: 1,
    features: { external_libraries: true, downloads: true, adult_metadata: true, remote_access: true },
  });
  assert.equal(isFeatureUnlocked(trial, "remote_access"), true);
  assert.equal(planLabel(trial), "CinaVault Plus trial · 1 day left");

  assert.equal(isFeatureUnlocked(null, "downloads"), true);
  assert.equal(normalizeEntitlements({ plan: "bogus" }).plan, "free");
  assert.equal(normalizeEntitlements(undefined).features.downloads, false);
});

test("paywall errors open the shared upgrade prompt instead of raw text", () => {
  useEntitlementsStore.getState().dismissPaywall();
  assert.equal(handlePaywallError("plain failure"), false);
  assert.equal(useEntitlementsStore.getState().paywallPrompt, null);

  assert.equal(handlePaywallError("PAYWALL:downloads:Need Plus"), true);
  assert.deepEqual(useEntitlementsStore.getState().paywallPrompt, {
    feature: "downloads",
    message: "Need Plus",
  });
  useEntitlementsStore.getState().dismissPaywall();

  const friendly = paywallAwareErrorMessage(new Error("PAYWALL:external_libraries:NAS needs Plus"));
  assert.equal(friendly, "NAS, cloud & network libraries needs CinaVault Plus");
  assert.equal(useEntitlementsStore.getState().paywallPrompt.feature, "external_libraries");
  assert.equal(paywallAwareErrorMessage("disk full"), "disk full");
  useEntitlementsStore.getState().dismissPaywall();
});

test("Plus gates are applied to the locked surfaces and the Account tab is registered", () => {
  const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
  const gates = {
    "src/components/tabs/CloudNASTab.tsx": 'feature="external_libraries"',
    "src/components/tabs/DownloadsTab.tsx": 'feature="downloads"',
    "src/components/tabs/RemoteAccessTab.tsx": 'feature="remote_access"',
    "src/components/tabs/AIDiagnosticsTab.tsx": 'feature="adult_metadata"',
    "src/components/tabs/PluginsTab.tsx": 'feature="adult_metadata"',
  };
  for (const [file, token] of Object.entries(gates)) {
    assert.ok(read(file).includes(token), `${file} is missing ${token}`);
  }
  const app = read("src/App.tsx");
  assert.match(app, /account: AccountTab/);
  assert.match(app, /<PaywallHost \/>/);
  assert.match(app, /<FirstRunSetup \/>/);
  assert.match(read("src/components/Sidebar.tsx"), /id: "account"/);
  assert.match(read("src/components/tabs/AccountTab.tsx"), /setActiveTab\("remote"\)/);
  assert.match(read("src/components/paywall/PaywallPanel.tsx"), /open_external_url|openCheckout/);
});

test("trial_available is offered only on the free plan", () => {
  const fresh = normalizeEntitlements({ plan: "free", trial_available: true, features: {} });
  assert.equal(fresh.trial_available, true);
  assert.equal(fresh.features.downloads, false);
  const running = normalizeEntitlements({ plan: "trial", trial_available: true, trial_days_left: 30 });
  assert.equal(running.trial_available, false);
  const used = normalizeEntitlements({ plan: "free", trial_days_left: 0 });
  assert.equal(used.trial_available, false);
});

test("the paywall panel offers the opt-in 30-day trial through start_trial", () => {
  const panel = readFileSync(new URL("../src/components/paywall/PaywallPanel.tsx", import.meta.url), "utf8");
  const service = readFileSync(new URL("../src/services/entitlements.ts", import.meta.url), "utf8");
  assert.match(panel, /trialAvailable && \(/);
  assert.match(panel, /Start free \$\{PLUS_TRIAL_DAYS\}-day trial/);
  assert.match(service, /invoke<unknown>\("start_trial"\)/);
  assert.match(service, /export const PLUS_TRIAL_DAYS = 30;/);
});

test("a paywall refusal is silent only when a gate is actually showing the lock", () => {
  const free = normalizeEntitlements({ plan: "free", features: { remote_access: false } });
  const plus = normalizeEntitlements({ plan: "plus", features: { remote_access: true } });
  const refusal = "PAYWALL:remote_access:Remote access needs Plus";
  assert.equal(paywallShownByGate(refusal, free), true);
  // Unknown plan: gates fail open, so the refusal must be surfaced.
  assert.equal(paywallShownByGate(refusal, null), false);
  assert.equal(paywallShownByGate(refusal, plus), false);
  assert.equal(paywallShownByGate("disk full", free), false);
});

test("a slow entitlement refresh never overwrites a newer activation", async () => {
  const pending = [];
  globalThis.window = {
    __TAURI_INTERNALS__: {
      invoke: (cmd) => new Promise((resolve) => pending.push({ cmd, resolve })),
    },
  };
  try {
    const store = useEntitlementsStore;
    const refresh = store.getState().refresh();
    const activate = store.getState().activate("CVL1.token.sig");
    await new Promise((r) => setImmediate(r));
    const byCmd = Object.fromEntries(pending.map((p) => [p.cmd, p.resolve]));
    // Activation answers first, then the stale refresh (taken before activation).
    byCmd.activate_license({ plan: "plus", features: { downloads: true } });
    await activate;
    byCmd.get_entitlements({ plan: "free", features: { downloads: false } });
    await refresh;
    assert.equal(store.getState().entitlements.plan, "plus");
    assert.equal(store.getState().status, "ready");
  } finally {
    delete globalThis.window;
    useEntitlementsStore.setState({ entitlements: null, status: "idle", error: null, paywallPrompt: null });
  }
});
