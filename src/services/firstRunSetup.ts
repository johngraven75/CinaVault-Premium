// First-run setup wizard — pure step, provider and validation logic.
//
// Kept free of Tauri/React/edition imports so it can be unit tested under
// node. Adult providers are supplied by the caller (see
// firstRunAdultProviders.ts) so the store-safe bundle can drop them at
// compile time instead of filtering at runtime.

export const FIRST_RUN_SETTING_KEY = "first_run_setup_complete";

export const FIRST_RUN_STEPS = ["welcome", "metadata", "vision", "done"] as const;
export type FirstRunStep = (typeof FIRST_RUN_STEPS)[number];

export interface SetupProvider {
  /** Provider id understood by set_api_key / test_api_key. */
  id: string;
  name: string;
  description: string;
  signupUrl: string;
  adult: boolean;
  /** Minimum plausible key length, used for client-side validation only. */
  minLength: number;
  optional: boolean;
}

export interface KeylessProvider {
  id: string;
  name: string;
  description: string;
  adult: boolean;
}

export const MAINSTREAM_SETUP_PROVIDERS: readonly SetupProvider[] = [
  {
    id: "tmdb",
    name: "TMDb",
    description: "Movie & TV posters, backdrops, cast and ratings.",
    signupUrl: "https://www.themoviedb.org/settings/api",
    adult: false,
    minLength: 16,
    optional: true,
  },
  {
    id: "omdb",
    name: "OMDb",
    description: "IMDb ratings, plots and release details.",
    signupUrl: "https://www.omdbapi.com/apikey.aspx",
    adult: false,
    minLength: 8,
    optional: true,
  },
  {
    id: "fanart",
    name: "Fanart.tv",
    description: "High-resolution logos, clear art and extra backdrops.",
    signupUrl: "https://fanart.tv/get-an-api-key/",
    adult: false,
    minLength: 16,
    optional: true,
  },
];

export const MAINSTREAM_KEYLESS_PROVIDERS: readonly KeylessProvider[] = [
  {
    id: "tvmaze",
    name: "TVMaze",
    description: "TV series, seasons and episode guides.",
    adult: false,
  },
  {
    id: "cinemeta",
    name: "Cinemeta",
    description: "Movie and series matching with posters, no account needed.",
    adult: false,
  },
];

/** Providers shown on the metadata step; adult entries never survive store-safe. */
export function buildSetupProviders(
  storeSafe: boolean,
  adultProviders: readonly SetupProvider[] = [],
): SetupProvider[] {
  const adult = storeSafe ? [] : adultProviders.filter((provider) => provider.adult);
  return [...adult, ...MAINSTREAM_SETUP_PROVIDERS];
}

export function buildKeylessProviders(
  storeSafe: boolean,
  adultKeyless: readonly KeylessProvider[] = [],
): KeylessProvider[] {
  const adult = storeSafe ? [] : adultKeyless.filter((provider) => provider.adult);
  return [...adult, ...MAINSTREAM_KEYLESS_PROVIDERS];
}

export function isFirstRunComplete(value: string | null | undefined): boolean {
  return (value ?? "").trim().toLowerCase() === "true";
}

export function shouldShowFirstRunSetup(value: string | null | undefined): boolean {
  return !isFirstRunComplete(value);
}

export function stepIndex(step: FirstRunStep): number {
  return FIRST_RUN_STEPS.indexOf(step);
}

export function nextStep(step: FirstRunStep): FirstRunStep {
  const index = stepIndex(step);
  return FIRST_RUN_STEPS[Math.min(FIRST_RUN_STEPS.length - 1, index + 1)];
}

export function previousStep(step: FirstRunStep): FirstRunStep {
  const index = stepIndex(step);
  return FIRST_RUN_STEPS[Math.max(0, index - 1)];
}

export function isLastStep(step: FirstRunStep): boolean {
  return stepIndex(step) === FIRST_RUN_STEPS.length - 1;
}

export interface KeyValidation {
  ok: boolean;
  message: string | null;
}

export function validateApiKeyInput(
  provider: Pick<SetupProvider, "name" | "minLength">,
  value: string,
): KeyValidation {
  const trimmed = value.trim();
  if (!trimmed) return { ok: false, message: `Paste your ${provider.name} key first.` };
  if (/\s/.test(trimmed)) {
    return { ok: false, message: "Keys can't contain spaces or line breaks." };
  }
  if (trimmed.length < provider.minLength) {
    return {
      ok: false,
      message: `That looks too short for a ${provider.name} key (at least ${provider.minLength} characters).`,
    };
  }
  return { ok: true, message: null };
}

export type KeyStatus =
  | "empty"
  | "saved"
  | "testing"
  | "valid"
  | "invalid"
  | "error";

export interface ProviderKeyState {
  value: string;
  status: KeyStatus;
  message: string | null;
}

export function initialKeyState(alreadyConfigured: boolean): ProviderKeyState {
  return {
    value: "",
    status: alreadyConfigured ? "saved" : "empty",
    message: alreadyConfigured ? "A key is already saved." : null,
  };
}

/** Interprets a test_api_key response ({ provider, valid }). */
export function interpretTestResult(
  provider: Pick<SetupProvider, "name">,
  result: unknown,
): { status: KeyStatus; message: string } {
  const valid =
    !!result &&
    typeof result === "object" &&
    (result as { valid?: unknown }).valid === true;
  return valid
    ? { status: "valid", message: `${provider.name} key verified and saved.` }
    : {
        status: "invalid",
        message: `${provider.name} rejected this key. Check it and try again.`,
      };
}

/** Provider ids that already have a key according to get_api_keys (masked map). */
export function configuredProviderIds(apiKeys: unknown): Set<string> {
  const ids = new Set<string>();
  if (!apiKeys || typeof apiKeys !== "object") return ids;
  for (const [provider, masked] of Object.entries(apiKeys as Record<string, unknown>)) {
    if (typeof masked === "string" && masked.trim()) ids.add(provider.toLowerCase());
  }
  return ids;
}

export function maskKey(value: string): string {
  const trimmed = value.trim();
  if (trimmed.length <= 4) return "••••";
  return `${trimmed.slice(0, 2)}${"•".repeat(Math.min(12, trimmed.length - 4))}${trimmed.slice(-2)}`;
}

/**
 * Runs saves for the same provider one after another, in the order they were
 * requested, so an older save can never land after a newer one and overwrite
 * it. `latest` tells the caller whether its value is still the newest request,
 * so stale results don't touch the UI.
 */
export function createSerialSaver(
  save: (providerId: string, value: string) => Promise<boolean>,
): (providerId: string, value: string) => Promise<{ saved: boolean; latest: boolean }> {
  const tails = new Map<string, Promise<unknown>>();
  const generations = new Map<string, number>();
  return (providerId, value) => {
    const generation = (generations.get(providerId) ?? 0) + 1;
    generations.set(providerId, generation);
    const previous = tails.get(providerId) ?? Promise.resolve();
    const run = previous
      .catch(() => undefined)
      .then(() => save(providerId, value))
      .then((saved) => ({ saved, latest: generations.get(providerId) === generation }));
    tails.set(providerId, run);
    return run;
  };
}

/** Extra sentence for enrichment results when Plus-only adult providers were left out. */
export function adultProvidersSkippedNote(result: { adult_providers_skipped?: string | null } | null | undefined): string {
  return result?.adult_providers_skipped
    ? " Adult metadata providers were skipped because they need CinaVault Plus."
    : "";
}
