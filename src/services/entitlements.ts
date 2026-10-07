// CinaVault Plus entitlements — front-end bridge for src-tauri/src/entitlements.rs.
//
// The Rust side is the authority: every Plus-only command refuses with an
// error that starts with "PAYWALL:<feature>:". The UI uses the entitlement
// snapshot only to decide what to show (lock badges, upgrade panels), and it
// fails open when the snapshot is unavailable so an older back end never
// hides features it would still serve.
import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";

export type PlusFeature =
  | "external_libraries"
  | "downloads"
  | "adult_metadata"
  | "remote_access";

export type PlanId = "free" | "trial" | "plus";

export interface Entitlements {
  plan: PlanId;
  trial_days_left: number | null;
  trial_ends_at: string | null;
  license_email: string | null;
  license_expires_at: string | null;
  features: Record<PlusFeature, boolean>;
  checkout_url: string | null;
  price_label: string;
  licensing_configured: boolean;
  /** True when this install has never used its one opt-in trial. */
  trial_available: boolean;
}

export interface PlusFeatureInfo {
  id: PlusFeature;
  label: string;
  description: string;
}

export const PLUS_PLAN_NAME = "CinaVault Plus";
export const PLUS_PRICE_LABEL = "$9.99/month";
export const PAYWALL_ERROR_PREFIX = "PAYWALL:";
export const PLUS_TRIAL_DAYS = 30;

export const PLUS_FEATURES: readonly PlusFeatureInfo[] = [
  {
    id: "external_libraries",
    label: "NAS, cloud & network libraries",
    description:
      "Add Synology, WD My Cloud, SMB/NFS shares and cloud drives to the unified library.",
  },
  {
    id: "downloads",
    label: "Downloads",
    description:
      "Download web media and HLS streams straight into the managed library.",
  },
  {
    id: "adult_metadata",
    label: "Adult metadata providers",
    description:
      "Automatic adult scene, performer and artwork matching from specialist providers.",
  },
  {
    id: "remote_access",
    label: "Remote access",
    description:
      "Stream your library from outside your home network with automatic relay.",
  },
];

/** Plus features to list in the UI; the store-safe edition never shows adult ones. */
export function visiblePlusFeatures(storeSafe: boolean): PlusFeatureInfo[] {
  return PLUS_FEATURES.filter(
    (feature) => !storeSafe || feature.id !== "adult_metadata",
  );
}

const FEATURE_ALIASES: Record<string, PlusFeature> = {
  external_libraries: "external_libraries",
  external_library: "external_libraries",
  nas: "external_libraries",
  cloud: "external_libraries",
  network_shares: "external_libraries",
  downloads: "downloads",
  download: "downloads",
  adult_metadata: "adult_metadata",
  adult: "adult_metadata",
  remote_access: "remote_access",
  remote: "remote_access",
};

export function isPlusFeature(value: unknown): value is PlusFeature {
  return (
    typeof value === "string" &&
    PLUS_FEATURES.some((feature) => feature.id === value)
  );
}

/** Maps a back-end feature token (case/alias tolerant) to a known Plus feature. */
export function normalizePlusFeature(value: string): PlusFeature | null {
  const key = value.trim().toLowerCase().replace(/[\s-]+/g, "_");
  return FEATURE_ALIASES[key] ?? null;
}

export function plusFeatureInfo(feature: PlusFeature): PlusFeatureInfo {
  return PLUS_FEATURES.find((item) => item.id === feature) ?? PLUS_FEATURES[0];
}

export function plusFeatureLabel(feature: string): string {
  const normalized = normalizePlusFeature(feature);
  return normalized ? plusFeatureInfo(normalized).label : feature;
}

function errorToText(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  if (error && typeof error === "object" && "message" in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string") return message;
  }
  return "";
}

export interface PaywallError {
  feature: PlusFeature;
  message: string;
}

/**
 * Parses a rejected invoke() value. Returns null unless it is a
 * "PAYWALL:<feature>:<message>" refusal for a known Plus feature.
 */
export function parsePaywallError(error: unknown): PaywallError | null {
  const text = errorToText(error).trim();
  if (!text.startsWith(PAYWALL_ERROR_PREFIX)) return null;
  const rest = text.slice(PAYWALL_ERROR_PREFIX.length);
  const separator = rest.indexOf(":");
  const rawFeature = separator === -1 ? rest : rest.slice(0, separator);
  const feature = normalizePlusFeature(rawFeature);
  if (!feature) return null;
  const detail = separator === -1 ? "" : rest.slice(separator + 1).trim();
  return {
    feature,
    message:
      detail ||
      `${plusFeatureInfo(feature).label} ${feature === "downloads" ? "require" : "requires"} ${PLUS_PLAN_NAME}.`,
  };
}

/** True when the feature may be shown unlocked. Unknown state fails open. */
export function isFeatureUnlocked(
  entitlements: Entitlements | null,
  feature: PlusFeature,
): boolean {
  if (!entitlements) return true;
  return entitlements.features?.[feature] !== false;
}

export function planLabel(entitlements: Entitlements | null): string {
  if (!entitlements) return "Checking plan…";
  if (entitlements.plan === "plus") return PLUS_PLAN_NAME;
  if (entitlements.plan === "trial") {
    const days = entitlements.trial_days_left ?? 0;
    return `${PLUS_PLAN_NAME} trial · ${days} day${days === 1 ? "" : "s"} left`;
  }
  return "CinaVault Free";
}

export function normalizeEntitlements(value: unknown): Entitlements {
  const raw = (value && typeof value === "object" ? value : {}) as Partial<Entitlements>;
  const features = (raw.features ?? {}) as Partial<Record<PlusFeature, boolean>>;
  const plan: PlanId =
    raw.plan === "plus" || raw.plan === "trial" ? raw.plan : "free";
  return {
    plan,
    trial_days_left:
      typeof raw.trial_days_left === "number" ? raw.trial_days_left : null,
    trial_ends_at: raw.trial_ends_at ?? null,
    license_email: raw.license_email ?? null,
    license_expires_at: raw.license_expires_at ?? null,
    features: {
      external_libraries: features.external_libraries === true,
      downloads: features.downloads === true,
      adult_metadata: features.adult_metadata === true,
      remote_access: features.remote_access === true,
    },
    checkout_url:
      typeof raw.checkout_url === "string" && raw.checkout_url.trim()
        ? raw.checkout_url
        : null,
    price_label: raw.price_label || PLUS_PRICE_LABEL,
    licensing_configured: raw.licensing_configured === true,
    trial_available: raw.trial_available === true && plan === "free",
  };
}

// ── Tauri commands ────────────────────────────────────────────────────────

export async function getEntitlements(): Promise<Entitlements> {
  return normalizeEntitlements(await invoke<unknown>("get_entitlements"));
}

export async function activateLicense(token: string): Promise<Entitlements> {
  return normalizeEntitlements(
    await invoke<unknown>("activate_license", { token: token.trim() }),
  );
}

export async function startTrial(): Promise<Entitlements> {
  return normalizeEntitlements(await invoke<unknown>("start_trial"));
}

export async function deactivateLicense(): Promise<Entitlements> {
  return normalizeEntitlements(await invoke<unknown>("deactivate_license"));
}

export async function openCheckout(url: string): Promise<void> {
  await invoke("open_external_url", { url });
}

// ── Shared state ──────────────────────────────────────────────────────────

export type EntitlementStatus = "idle" | "loading" | "ready" | "unavailable";

interface EntitlementState {
  entitlements: Entitlements | null;
  status: EntitlementStatus;
  error: string | null;
  /** Feature whose upgrade panel is currently requested (global modal). */
  paywallPrompt: PaywallError | null;
  refresh: () => Promise<Entitlements | null>;
  activate: (token: string) => Promise<Entitlements>;
  deactivate: () => Promise<Entitlements>;
  startTrial: () => Promise<Entitlements>;
  showPaywall: (feature: PlusFeature, message?: string) => void;
  dismissPaywall: () => void;
}

// Bumped by every request that can change entitlements. A response is applied
// only if no newer request started after it, so a slow refresh can never
// overwrite the plan a later activate/trial/deactivate just set.
let entitlementVersion = 0;

export const useEntitlementsStore = create<EntitlementState>((set) => ({
  entitlements: null,
  status: "idle",
  error: null,
  paywallPrompt: null,
  refresh: async () => {
    const version = ++entitlementVersion;
    set({ status: "loading" });
    try {
      const entitlements = await getEntitlements();
      if (version === entitlementVersion) set({ entitlements, status: "ready", error: null });
      return entitlements;
    } catch (error) {
      // Older back ends have no entitlement command: keep the UI unlocked and
      // let the back end remain the authority.
      if (version === entitlementVersion) {
        set({ entitlements: null, status: "unavailable", error: errorToText(error) });
      }
      return null;
    }
  },
  activate: async (token) => {
    const version = ++entitlementVersion;
    const entitlements = await activateLicense(token);
    if (version === entitlementVersion) set({ entitlements, status: "ready", error: null });
    return entitlements;
  },
  deactivate: async () => {
    const version = ++entitlementVersion;
    const entitlements = await deactivateLicense();
    if (version === entitlementVersion) set({ entitlements, status: "ready", error: null });
    return entitlements;
  },
  startTrial: async () => {
    const version = ++entitlementVersion;
    const entitlements = await startTrial();
    if (version === entitlementVersion) {
      set({ entitlements, status: "ready", error: null, paywallPrompt: null });
    }
    return entitlements;
  },
  showPaywall: (feature, message) =>
    set({
      paywallPrompt: {
        feature,
        message:
          message || `${plusFeatureInfo(feature).label} is part of ${PLUS_PLAN_NAME}.`,
      },
    }),
  dismissPaywall: () => set({ paywallPrompt: null }),
}));

/** Hook: whether a Plus feature is currently unlocked (fails open while unknown). */
export function useFeatureUnlocked(feature: PlusFeature): boolean {
  return useEntitlementsStore((state) =>
    isFeatureUnlocked(state.entitlements, feature),
  );
}

/**
 * For catch blocks around invoke(): when the error is a PAYWALL refusal it
 * opens the upgrade panel and returns true, so the caller can skip its raw
 * error output.
 */
export function handlePaywallError(error: unknown): boolean {
  const paywall = parsePaywallError(error);
  if (!paywall) return false;
  useEntitlementsStore.getState().showPaywall(paywall.feature, paywall.message);
  return true;
}

/**
 * Human-readable error text for status lines. PAYWALL refusals open the
 * upgrade panel and become a short friendly sentence instead of raw text.
 */
/**
 * True when `error` is a Plus refusal that an on-screen FeatureGate is already
 * showing (the plan is known and the feature is locked), so background loads
 * can stay quiet. When entitlements are unknown the gates fail open, so the
 * refusal must be surfaced instead.
 */
export function paywallShownByGate(
  error: unknown,
  entitlements: Entitlements | null = useEntitlementsStore.getState().entitlements,
): boolean {
  const paywall = parsePaywallError(error);
  return paywall !== null && !isFeatureUnlocked(entitlements, paywall.feature);
}

export function paywallAwareErrorMessage(error: unknown): string {
  const paywall = parsePaywallError(error);
  if (paywall) {
    useEntitlementsStore.getState().showPaywall(paywall.feature, paywall.message);
    return `${plusFeatureInfo(paywall.feature).label} needs ${PLUS_PLAN_NAME}`;
  }
  return errorToText(error) || String(error);
}
