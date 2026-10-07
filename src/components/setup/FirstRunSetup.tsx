// First-run setup wizard: welcome → metadata keys → local AI vision → done.
// Shown once at boot until setting `first_run_setup_complete` is "true";
// reopened from Settings via the "cinavault:open-first-run-setup" event.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { JSX } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AnimatePresence, motion, useReducedMotion } from "framer-motion";
import {
  ArrowLeft,
  ArrowRight,
  CheckCircle2,
  Cpu,
  ExternalLink,
  Eye,
  EyeOff,
  KeyRound,
  Loader2,
  Rocket,
  ScanEye,
  ShieldCheck,
  Sparkles,
  XCircle,
  Zap,
} from "lucide-react";
import { IS_STORE_SAFE } from "../../config/edition";
import { useAppStore, type LibraryEnrichmentResult } from "../../store/appStore";
import {
  FIRST_RUN_SETTING_KEY,
  FIRST_RUN_STEPS,
  buildKeylessProviders,
  buildSetupProviders,
  configuredProviderIds,
  createSerialSaver,
  initialKeyState,
  interpretTestResult,
  isLastStep,
  nextStep,
  previousStep,
  shouldShowFirstRunSetup,
  stepIndex,
  validateApiKeyInput,
  type FirstRunStep,
  type ProviderKeyState,
  type SetupProvider,
} from "../../services/firstRunSetup";
import {
  ADULT_KEYLESS_PROVIDERS,
  ADULT_SETUP_PROVIDERS,
} from "../../services/firstRunAdultProviders";

export const OPEN_FIRST_RUN_SETUP_EVENT = "cinavault:open-first-run-setup";

// Resolved at build time: in the store-safe edition both lists are empty
// literals and the adult provider module is tree-shaken out of the bundle.
const SETUP_PROVIDERS = buildSetupProviders(
  IS_STORE_SAFE,
  IS_STORE_SAFE ? [] : ADULT_SETUP_PROVIDERS,
);
const KEYLESS_PROVIDERS = buildKeylessProviders(
  IS_STORE_SAFE,
  IS_STORE_SAFE ? [] : ADULT_KEYLESS_PROVIDERS,
);

const STEP_LABELS: Record<FirstRunStep, string> = {
  welcome: "Welcome",
  metadata: "Metadata",
  vision: "AI Vision",
  done: "Launch",
};

const AUTO_SAVE_DELAY_MS = 900;

function errorText(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

export function openFirstRunSetup(): void {
  window.dispatchEvent(new Event(OPEN_FIRST_RUN_SETUP_EVENT));
}

export default function FirstRunSetup(): JSX.Element {
  const reduceMotion = useReducedMotion();
  const setSetting = useAppStore((state) => state.setSetting);
  const addStatusMessage = useAppStore((state) => state.addStatusMessage);

  const [open, setOpen] = useState(false);
  const [step, setStep] = useState<FirstRunStep>("welcome");
  const [direction, setDirection] = useState(1);
  const [keys, setKeys] = useState<Record<string, ProviderKeyState>>(() =>
    Object.fromEntries(SETUP_PROVIDERS.map((p) => [p.id, initialKeyState(false)])),
  );
  const [revealed, setRevealed] = useState<Record<string, boolean>>({});
  const [finishing, setFinishing] = useState(false);
  const saveTimers = useRef<Record<string, number>>({});

  // Boot check: show once until the setting says complete.
  useEffect(() => {
    let active = true;
    invoke<string | null>("get_setting", { key: FIRST_RUN_SETTING_KEY })
      .then((value) => {
        if (active && shouldShowFirstRunSetup(value)) setOpen(true);
      })
      .catch(() => {
        // No settings back end (e.g. plain browser preview): never nag.
      });
    const reopen = () => {
      setStep("welcome");
      setDirection(1);
      setOpen(true);
    };
    window.addEventListener(OPEN_FIRST_RUN_SETUP_EVENT, reopen);
    return () => {
      active = false;
      window.removeEventListener(OPEN_FIRST_RUN_SETUP_EVENT, reopen);
    };
  }, []);

  // Load which providers already have keys when the wizard opens.
  useEffect(() => {
    if (!open) return;
    let active = true;
    invoke<unknown>("get_api_keys")
      .then((apiKeys) => {
        if (!active) return;
        const configured = configuredProviderIds(apiKeys);
        setKeys((current) => {
          const next = { ...current };
          for (const provider of SETUP_PROVIDERS) {
            const state = next[provider.id];
            if (configured.has(provider.id) && state && !state.value && state.status === "empty") {
              next[provider.id] = initialKeyState(true);
            }
          }
          return next;
        });
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, [open]);

  useEffect(
    () => () => {
      for (const timer of Object.values(saveTimers.current)) window.clearTimeout(timer);
    },
    [],
  );

  const patchKey = useCallback((id: string, patch: Partial<ProviderKeyState>) => {
    setKeys((current) => ({ ...current, [id]: { ...current[id], ...patch } }));
  }, []);

  // One queue for the wizard's lifetime: saves per provider run in order.
  const [serialSave] = useState(() =>
    createSerialSaver(async (providerId, value) => {
      await invoke("set_api_key", { provider: providerId, apiKey: value.trim() });
      return true;
    }),
  );

  /** Saves `value`; returns true only if it saved and is still the newest value for the provider. */
  const saveKey = useCallback(
    async (provider: SetupProvider, value: string): Promise<boolean> => {
      try {
        const { saved, latest } = await serialSave(provider.id, value);
        return saved && latest;
      } catch (error) {
        patchKey(provider.id, {
          status: "error",
          message: `Couldn't save: ${String(error)}`,
        });
        return false;
      }
    },
    [patchKey, serialSave],
  );

  const onKeyChange = (provider: SetupProvider, value: string) => {
    patchKey(provider.id, { value, status: "empty", message: null });
    window.clearTimeout(saveTimers.current[provider.id]);
    if (!value.trim()) return;
    saveTimers.current[provider.id] = window.setTimeout(() => {
      const check = validateApiKeyInput(provider, value);
      if (!check.ok) {
        patchKey(provider.id, { status: "error", message: check.message });
        return;
      }
      void saveKey(provider, value).then((saved) => {
        if (saved) patchKey(provider.id, { status: "saved", message: "Saved securely. Press Test to verify." });
      });
    }, AUTO_SAVE_DELAY_MS);
  };

  const testKey = async (provider: SetupProvider) => {
    const state = keys[provider.id];
    const value = state?.value ?? "";
    const check = validateApiKeyInput(provider, value);
    if (!check.ok) {
      patchKey(provider.id, { status: "error", message: check.message });
      return;
    }
    window.clearTimeout(saveTimers.current[provider.id]);
    patchKey(provider.id, { status: "testing", message: "Contacting provider…" });
    try {
      const result = await invoke<unknown>("test_api_key", {
        provider: provider.id,
        apiKey: value.trim(),
      });
      const outcome = interpretTestResult(provider, result);
      if (outcome.status === "valid") {
        const saved = await saveKey(provider, value);
        if (!saved) return;
      }
      patchKey(provider.id, outcome);
    } catch (error) {
      patchKey(provider.id, {
        status: "error",
        message: `Test failed: ${String(error)}`,
      });
    }
  };

  const markComplete = useCallback(async () => {
    setSetting(FIRST_RUN_SETTING_KEY, "true");
    try {
      await invoke("set_setting", { key: FIRST_RUN_SETTING_KEY, value: "true" });
    } catch (error) {
      addStatusMessage(`Setup state could not be saved: ${errorText(error)}`);
    }
  }, [addStatusMessage, setSetting]);

  const skip = async () => {
    setOpen(false);
    await markComplete();
    addStatusMessage("Setup skipped — you can reopen it any time from Settings");
  };

  const finish = async () => {
    setFinishing(true);
    await markComplete();
    setFinishing(false);
    setOpen(false);
    addStatusMessage("Library enrichment started: identifying titles and fetching posters…");
    // Fire and forget — the wizard never waits on a full-library pass.
    void invoke<LibraryEnrichmentResult>("run_library_enrichment", { renameFiles: false })
      .then((result) => {
        const enriched = result?.metadata_items_enriched ?? result?.metadata_updated ?? 0;
        addStatusMessage(
          `Library enrichment complete: ${enriched} items enriched, ${result?.posters_downloaded ?? 0} posters downloaded.`,
        );
        window.dispatchEvent(
          new CustomEvent("cinavault:library-refresh", { detail: { reason: "first-run-enrichment" } }),
        );
      })
      .catch((error) => {
        addStatusMessage(`Library enrichment could not run: ${String(error)}`);
      });
  };

  const go = (target: FirstRunStep) => {
    setDirection(stepIndex(target) >= stepIndex(step) ? 1 : -1);
    setStep(target);
  };

  const keyedCount = useMemo(
    () =>
      Object.values(keys).filter((state) =>
        ["saved", "valid"].includes(state.status),
      ).length,
    [keys],
  );

  const slide = reduceMotion
    ? { initial: { opacity: 0 }, animate: { opacity: 1 }, exit: { opacity: 0 } }
    : {
        initial: { opacity: 0, x: 48 * direction, filter: "blur(6px)" },
        animate: { opacity: 1, x: 0, filter: "blur(0px)" },
        exit: { opacity: 0, x: -48 * direction, filter: "blur(6px)" },
      };

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          key="cv-first-run"
          className="fixed inset-0 z-[150] flex items-center justify-center bg-[radial-gradient(circle_at_30%_20%,rgba(0,234,255,0.14),transparent_45%),radial-gradient(circle_at_80%_80%,rgba(217,70,239,0.16),transparent_45%),rgba(2,4,10,0.86)] p-4 backdrop-blur-md"
          role="dialog"
          aria-modal="true"
          aria-labelledby="cv-first-run-title"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          data-testid="cinavault-first-run-setup"
        >
          <motion.div
            className="glass-panel relative flex max-h-[92vh] w-full max-w-3xl flex-col overflow-hidden rounded-[28px] border border-cyan-200/20 shadow-[0_40px_120px_rgba(0,0,0,0.55),0_0_60px_rgba(0,234,255,0.12)]"
            initial={reduceMotion ? false : { opacity: 0, y: 30, scale: 0.96, rotateX: 4 }}
            animate={{ opacity: 1, y: 0, scale: 1, rotateX: 0 }}
            exit={reduceMotion ? { opacity: 0 } : { opacity: 0, y: -16, scale: 0.98 }}
            transition={{ duration: 0.45, ease: [0.16, 1, 0.3, 1] }}
            style={{ transformPerspective: 1200 }}
          >
            {/* Header: progress + skip */}
            <div className="relative z-10 flex items-center justify-between gap-3 border-b border-white/10 px-6 py-4">
              <ol className="flex items-center gap-2" aria-label="Setup progress">
                {FIRST_RUN_STEPS.map((item, index) => {
                  const current = item === step;
                  const done = index < stepIndex(step);
                  return (
                    <li key={item} className="flex items-center gap-2">
                      <span
                        aria-current={current ? "step" : undefined}
                        className={`grid h-6 w-6 place-items-center rounded-full border text-[10px] font-black transition-colors ${
                          current
                            ? "border-cyan-200 bg-cyan-300/25 text-cyan-50 shadow-[0_0_14px_rgba(0,234,255,0.5)]"
                            : done
                              ? "border-emerald-300/60 bg-emerald-400/20 text-emerald-100"
                              : "border-white/15 text-cv-subtext"
                        }`}
                      >
                        {done ? <CheckCircle2 size={12} /> : index + 1}
                      </span>
                      <span className={`hidden text-[10px] font-bold uppercase tracking-[0.18em] sm:inline ${current ? "text-cyan-50" : "text-cv-subtext"}`}>
                        {STEP_LABELS[item]}
                      </span>
                      {index < FIRST_RUN_STEPS.length - 1 && <span className="h-px w-4 bg-white/15 sm:w-6" aria-hidden="true" />}
                    </li>
                  );
                })}
              </ol>
              <button type="button" onClick={() => void skip()} className="text-[11px] font-semibold text-cv-subtext underline-offset-4 hover:text-cv-text hover:underline">
                Skip setup
              </button>
            </div>

            {/* Body */}
            <div className="relative z-10 min-h-0 flex-1 overflow-y-auto px-6 py-6">
              <AnimatePresence mode="wait" initial={false}>
                <motion.div
                  key={step}
                  initial={slide.initial}
                  animate={slide.animate}
                  exit={slide.exit}
                  transition={{ duration: reduceMotion ? 0.15 : 0.32, ease: [0.16, 1, 0.3, 1] }}
                >
                  {step === "welcome" && <WelcomeStep reduceMotion={!!reduceMotion} />}
                  {step === "metadata" && (
                    <MetadataStep
                      keys={keys}
                      revealed={revealed}
                      onToggleReveal={(id) => setRevealed((current) => ({ ...current, [id]: !current[id] }))}
                      onChange={onKeyChange}
                      onTest={(provider) => void testKey(provider)}
                    />
                  )}
                  {step === "vision" && <VisionStep />}
                  {step === "done" && <DoneStep keyedCount={keyedCount} />}
                </motion.div>
              </AnimatePresence>
            </div>

            {/* Footer navigation */}
            <div className="relative z-10 flex items-center justify-between gap-3 border-t border-white/10 px-6 py-4">
              <button
                type="button"
                onClick={() => go(previousStep(step))}
                disabled={step === "welcome"}
                className="cv-btn cv-btn-secondary text-xs disabled:invisible"
              >
                <ArrowLeft size={14} /> Back
              </button>
              {isLastStep(step) ? (
                <button type="button" onClick={() => void finish()} disabled={finishing} className="cv-btn cv-btn-primary text-sm disabled:opacity-60">
                  {finishing ? <Loader2 size={15} className="animate-spin" /> : <Rocket size={15} />} Enter the Vault
                </button>
              ) : (
                <button type="button" onClick={() => go(nextStep(step))} className="cv-btn cv-btn-primary text-sm">
                  {step === "welcome" ? "Get started" : "Continue"} <ArrowRight size={15} />
                </button>
              )}
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function WelcomeStep({ reduceMotion }: { reduceMotion: boolean }): JSX.Element {
  return (
    <div className="text-center">
      <motion.div
        className="mx-auto grid h-20 w-20 place-items-center rounded-[26px] border border-cyan-200/40 bg-[linear-gradient(135deg,rgba(0,234,255,0.25),rgba(217,70,239,0.25))] text-cyan-50"
        animate={reduceMotion ? undefined : { rotate: [0, 4, -4, 0], boxShadow: ["0 0 20px rgba(0,234,255,0.3)", "0 0 46px rgba(217,70,239,0.45)", "0 0 20px rgba(0,234,255,0.3)"] }}
        transition={{ duration: 5, repeat: Infinity, ease: "easeInOut" }}
      >
        <Sparkles size={34} aria-hidden="true" />
      </motion.div>
      <h2 id="cv-first-run-title" className="mt-5 text-3xl font-black tracking-tight text-white">
        Welcome to CinaVault
      </h2>
      <p className="mx-auto mt-3 max-w-lg text-sm leading-6 text-cv-subtext">
        Your media server is already running. Every source you add becomes one
        unified library — duplicates are merged into a single card, and titles,
        posters and artwork are identified automatically.
      </p>
      <div className="mx-auto mt-6 grid max-w-xl gap-3 text-left sm:grid-cols-3">
        {[
          { icon: Zap, title: "Zero config", text: "Free metadata sources work out of the box." },
          { icon: ScanEye, title: "Local AI vision", text: "Posters & frames identified on this PC." },
          { icon: ShieldCheck, title: "Private", text: "Keys are stored in the OS credential vault." },
        ].map(({ icon: Icon, title, text }) => (
          <div key={title} className="glass-panel-2 rounded-xl border border-white/10 p-3">
            <Icon size={16} className="text-cyan-200" aria-hidden="true" />
            <div className="mt-2 text-xs font-bold text-cv-text">{title}</div>
            <div className="mt-1 text-[11px] leading-4 text-cv-subtext">{text}</div>
          </div>
        ))}
      </div>
      <p className="mt-5 text-[11px] text-cv-subtext">Takes about a minute. Everything here is optional.</p>
    </div>
  );
}

function KeyStatusLine({ state }: { state: ProviderKeyState }): JSX.Element | null {
  if (!state.message) return null;
  const tone =
    state.status === "valid" || state.status === "saved"
      ? "text-emerald-200"
      : state.status === "invalid" || state.status === "error"
        ? "text-red-200"
        : "text-cv-subtext";
  const Icon =
    state.status === "valid" || state.status === "saved"
      ? CheckCircle2
      : state.status === "invalid" || state.status === "error"
        ? XCircle
        : Loader2;
  return (
    <p role="status" className={`mt-1.5 flex items-center gap-1.5 text-[10px] ${tone}`}>
      <Icon size={11} className={state.status === "testing" ? "animate-spin" : ""} aria-hidden="true" /> {state.message}
    </p>
  );
}

function MetadataStep({
  keys,
  revealed,
  onToggleReveal,
  onChange,
  onTest,
}: {
  keys: Record<string, ProviderKeyState>;
  revealed: Record<string, boolean>;
  onToggleReveal: (id: string) => void;
  onChange: (provider: SetupProvider, value: string) => void;
  onTest: (provider: SetupProvider) => void;
}): JSX.Element {
  const openSignup = (url: string) => {
    void invoke("open_external_url", { url }).catch(() => {
      window.open(url, "_blank", "noopener,noreferrer");
    });
  };

  return (
    <div>
      <h2 id="cv-first-run-title" className="text-2xl font-black tracking-tight text-white">
        Metadata providers
      </h2>
      <p className="mt-1 text-sm text-cv-subtext">
        Optional free keys unlock richer artwork. Keys save automatically as you paste them.
      </p>

      <div className="mt-4 rounded-2xl border border-emerald-300/20 bg-emerald-400/[0.06] p-3">
        <div className="text-[10px] font-bold uppercase tracking-[0.2em] text-emerald-200">
          Already active — no key needed
        </div>
        <div className="mt-2 flex flex-wrap gap-2">
          {KEYLESS_PROVIDERS.map((provider) => (
            <span
              key={provider.id}
              title={provider.description}
              className="inline-flex items-center gap-1.5 rounded-full border border-emerald-300/30 bg-emerald-400/10 px-2.5 py-1 text-[11px] font-semibold text-emerald-50"
            >
              <CheckCircle2 size={11} aria-hidden="true" /> {provider.name}
            </span>
          ))}
        </div>
      </div>

      <div className="mt-4 grid gap-3">
        {SETUP_PROVIDERS.map((provider) => {
          const state = keys[provider.id] ?? initialKeyState(false);
          const inputId = `cv-setup-key-${provider.id}`;
          return (
            <div key={provider.id} className="glass-panel-2 rounded-xl border border-white/10 p-3">
              <div className="flex flex-wrap items-start justify-between gap-2">
                <div>
                  <label htmlFor={inputId} className="flex items-center gap-1.5 text-sm font-bold text-cv-text">
                    <KeyRound size={13} className="text-cyan-200" aria-hidden="true" /> {provider.name}
                    {(state.status === "valid" || state.status === "saved") && (
                      <CheckCircle2 size={13} className="text-emerald-300" aria-label="configured" />
                    )}
                  </label>
                  <p className="mt-0.5 text-[11px] text-cv-subtext">{provider.description}</p>
                </div>
                <button type="button" onClick={() => openSignup(provider.signupUrl)} className="inline-flex items-center gap-1 text-[11px] font-semibold text-cyan-200 hover:underline">
                  Get a free key <ExternalLink size={11} aria-hidden="true" />
                </button>
              </div>
              <div className="mt-2 flex gap-2">
                <div className="relative min-w-0 flex-1">
                  <input
                    id={inputId}
                    type={revealed[provider.id] ? "text" : "password"}
                    autoComplete="off"
                    spellCheck={false}
                    value={state.value}
                    onChange={(event) => onChange(provider, event.target.value)}
                    placeholder={state.status === "saved" && !state.value ? "Key saved — paste to replace" : "Paste API key"}
                    className="cv-input w-full pr-9 font-mono text-xs"
                  />
                  <button
                    type="button"
                    onClick={() => onToggleReveal(provider.id)}
                    className="absolute right-2 top-1/2 -translate-y-1/2 text-cv-subtext hover:text-cv-text"
                    aria-label={revealed[provider.id] ? `Hide ${provider.name} key` : `Show ${provider.name} key`}
                  >
                    {revealed[provider.id] ? <EyeOff size={13} /> : <Eye size={13} />}
                  </button>
                </div>
                <button
                  type="button"
                  onClick={() => onTest(provider)}
                  disabled={state.status === "testing" || !state.value.trim()}
                  className="cv-btn cv-btn-secondary shrink-0 text-xs disabled:opacity-50"
                >
                  {state.status === "testing" ? <Loader2 size={12} className="animate-spin" /> : <Zap size={12} />} Test
                </button>
              </div>
              <KeyStatusLine state={state} />
            </div>
          );
        })}
      </div>
    </div>
  );
}

function VisionStep(): JSX.Element {
  return (
    <div className="text-center">
      <div className="mx-auto grid h-16 w-16 place-items-center rounded-2xl border border-fuchsia-200/40 bg-fuchsia-400/15 text-fuchsia-100 shadow-[0_0_30px_rgba(217,70,239,0.3)]">
        <Cpu size={28} aria-hidden="true" />
      </div>
      <h2 id="cv-first-run-title" className="mt-4 text-2xl font-black tracking-tight text-white">
        AI vision is ready
      </h2>
      <p className="mx-auto mt-2 max-w-lg text-sm leading-6 text-cv-subtext">
        Poster and film identification runs locally with a free, bundled vision
        model. Nothing to set up — no account, no API key, and your media never
        leaves this computer.
      </p>
      <div className="mx-auto mt-5 grid max-w-lg gap-2 text-left">
        {[
          "Matches posters and video frames to the right title",
          "Fills in missing artwork for files with unclear names",
          "Works offline once the model is on disk",
        ].map((line) => (
          <div key={line} className="flex items-center gap-2 rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2 text-xs text-cv-text">
            <CheckCircle2 size={13} className="shrink-0 text-emerald-300" aria-hidden="true" /> {line}
          </div>
        ))}
      </div>
    </div>
  );
}

function DoneStep({ keyedCount }: { keyedCount: number }): JSX.Element {
  return (
    <div className="text-center">
      <div className="mx-auto grid h-16 w-16 place-items-center rounded-full border border-emerald-200/50 bg-emerald-400/15 text-emerald-100 shadow-[0_0_34px_rgba(52,211,153,0.35)]">
        <Rocket size={28} aria-hidden="true" />
      </div>
      <h2 id="cv-first-run-title" className="mt-4 text-2xl font-black tracking-tight text-white">
        You're all set
      </h2>
      <p className="mx-auto mt-2 max-w-lg text-sm leading-6 text-cv-subtext">
        {KEYLESS_PROVIDERS.length} free sources are active
        {keyedCount > 0 ? ` and ${keyedCount} provider key${keyedCount === 1 ? " is" : "s are"} saved` : ""}.
        When you enter the vault, CinaVault starts identifying your library and
        fetching posters in the background.
      </p>
      <p className="mt-3 text-[11px] text-cv-subtext">You can rerun this setup any time from Settings.</p>
    </div>
  );
}
