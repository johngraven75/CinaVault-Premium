// CinaVault Plus upgrade panel: plan, price, Plus features, checkout and
// offline license activation. Used inline (AccountTab) and in the global
// paywall modal (PaywallHost) whenever a command refuses with "PAYWALL:".
import { useEffect, useId, useState } from "react";
import type { JSX } from "react";
import { motion, useReducedMotion } from "framer-motion";
import {
  CheckCircle2,
  Crown,
  ExternalLink,
  KeyRound,
  Loader2,
  Lock,
  Sparkles,
  X,
} from "lucide-react";
import {
  visiblePlusFeatures,
  PLUS_PLAN_NAME,
  PLUS_PRICE_LABEL,
  PLUS_TRIAL_DAYS,
  isFeatureUnlocked,
  openCheckout,
  planLabel,
  useEntitlementsStore,
  type PlusFeature,
} from "../../services/entitlements";
import { IS_STORE_SAFE } from "../../config/edition";

export interface PaywallPanelProps {
  /** Feature that triggered the panel; highlighted in the list. */
  feature?: PlusFeature | null;
  /** Message from the back end (PAYWALL detail) shown above the offer. */
  message?: string | null;
  onClose?: () => void;
  /** Called after a successful activation. */
  onActivated?: () => void;
}

function errorText(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

export default function PaywallPanel({
  feature = null,
  message = null,
  onClose,
  onActivated,
}: PaywallPanelProps): JSX.Element {
  const reduceMotion = useReducedMotion();
  const entitlements = useEntitlementsStore((state) => state.entitlements);
  const status = useEntitlementsStore((state) => state.status);
  const refresh = useEntitlementsStore((state) => state.refresh);
  const activate = useEntitlementsStore((state) => state.activate);
  const beginTrial = useEntitlementsStore((state) => state.startTrial);

  const licenseInputId = useId();
  const [token, setToken] = useState("");
  const [busy, setBusy] = useState<"activate" | "checkout" | "trial" | null>(null);
  const [notice, setNotice] = useState<{ kind: "ok" | "error"; text: string } | null>(null);

  useEffect(() => {
    if (status === "idle") void refresh();
  }, [status, refresh]);

  const checkoutUrl = entitlements?.checkout_url ?? null;
  const price = entitlements?.price_label || PLUS_PRICE_LABEL;
  const trialDays =
    entitlements?.plan === "trial" ? entitlements.trial_days_left ?? 0 : null;
  const isPlus = entitlements?.plan === "plus";

  const startCheckout = async () => {
    if (!checkoutUrl) return;
    setBusy("checkout");
    setNotice(null);
    try {
      await openCheckout(checkoutUrl);
      setNotice({
        kind: "ok",
        text: "Checkout opened in your browser. Paste the license key you receive below.",
      });
    } catch (error) {
      setNotice({ kind: "error", text: `Couldn't open checkout: ${errorText(error)}` });
    } finally {
      setBusy(null);
    }
  };

  const trialAvailable = entitlements?.trial_available === true;

  const startFreeTrial = async () => {
    setBusy("trial");
    setNotice(null);
    try {
      const next = await beginTrial();
      setNotice({
        kind: "ok",
        text: `Your ${PLUS_TRIAL_DAYS}-day ${PLUS_PLAN_NAME} trial is on${
          next.trial_ends_at ? ` until ${new Date(next.trial_ends_at).toLocaleDateString()}` : ""
        }.`,
      });
      onActivated?.();
    } catch (error) {
      setNotice({ kind: "error", text: errorText(error) || "The trial couldn't be started." });
    } finally {
      setBusy(null);
    }
  };

  const activateToken = async () => {
    const trimmed = token.trim();
    if (!trimmed) {
      setNotice({ kind: "error", text: "Paste your CinaVault Plus license key first." });
      return;
    }
    setBusy("activate");
    setNotice(null);
    try {
      const next = await activate(trimmed);
      setToken("");
      setNotice({
        kind: "ok",
        text:
          next.plan === "plus"
            ? `${PLUS_PLAN_NAME} activated${next.license_email ? ` for ${next.license_email}` : ""}.`
            : "License accepted.",
      });
      onActivated?.();
    } catch (error) {
      setNotice({ kind: "error", text: errorText(error) || "That license key was not accepted." });
    } finally {
      setBusy(null);
    }
  };

  return (
    <motion.div
      className="cv-paywall-panel relative overflow-hidden rounded-[24px] border border-fuchsia-300/25 bg-[linear-gradient(140deg,rgba(24,10,40,0.94),rgba(6,14,28,0.94))] p-5 shadow-[0_30px_90px_rgba(120,40,200,0.28)]"
      initial={reduceMotion ? false : { opacity: 0, y: 16, scale: 0.98 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      transition={{ duration: 0.3, ease: [0.16, 1, 0.3, 1] }}
      data-testid="cinavault-paywall-panel"
    >
      <div
        aria-hidden="true"
        className="pointer-events-none absolute -right-20 -top-24 h-64 w-64 rounded-full bg-fuchsia-500/25 blur-3xl"
      />
      <div
        aria-hidden="true"
        className="pointer-events-none absolute -bottom-24 -left-16 h-56 w-56 rounded-full bg-cyan-400/15 blur-3xl"
      />

      <div className="relative z-10">
        <div className="flex items-start justify-between gap-3">
          <div className="flex items-center gap-3">
            <span className="grid h-12 w-12 place-items-center rounded-2xl border border-amber-200/40 bg-amber-300/15 text-amber-200 shadow-[0_0_24px_rgba(252,211,77,0.3)]">
              <Crown size={22} aria-hidden="true" />
            </span>
            <div>
              <div className="text-[10px] font-bold uppercase tracking-[0.3em] text-fuchsia-200/80">
                Upgrade
              </div>
              <h3 className="text-xl font-black tracking-tight text-white">{PLUS_PLAN_NAME}</h3>
              <div className="text-sm font-semibold text-amber-200">{price}</div>
            </div>
          </div>
          {onClose && (
            <button
              type="button"
              onClick={onClose}
              className="cv-btn cv-btn-secondary h-9 w-9 px-0"
              aria-label="Close upgrade panel"
            >
              <X size={15} />
            </button>
          )}
        </div>

        {message && (
          <p className="mt-4 flex items-start gap-2 rounded-xl border border-amber-200/25 bg-amber-300/10 px-3 py-2 text-xs text-amber-100">
            <Lock size={13} className="mt-0.5 shrink-0" aria-hidden="true" /> {message}
          </p>
        )}

        <div className="mt-3 text-xs text-cv-subtext">
          Current plan: <span className="font-semibold text-cv-text">{planLabel(entitlements)}</span>
          {trialDays !== null && (
            <span className="ml-2 rounded-full bg-cyan-300/15 px-2 py-0.5 text-[10px] font-bold text-cyan-100">
              {trialDays} trial day{trialDays === 1 ? "" : "s"} left
            </span>
          )}
        </div>

        <ul className="mt-4 grid gap-2 sm:grid-cols-2">
          {visiblePlusFeatures(IS_STORE_SAFE).map((item) => {
            const unlocked = isFeatureUnlocked(entitlements, item.id) && entitlements !== null;
            const highlighted = item.id === feature;
            return (
              <li
                key={item.id}
                className={`rounded-xl border px-3 py-2.5 ${
                  highlighted
                    ? "border-fuchsia-300/60 bg-fuchsia-400/15 shadow-[0_0_20px_rgba(217,70,239,0.25)]"
                    : "border-white/10 bg-white/[0.03]"
                }`}
              >
                <div className="flex items-center gap-2 text-xs font-bold text-cv-text">
                  {unlocked ? (
                    <CheckCircle2 size={13} className="text-emerald-300" aria-hidden="true" />
                  ) : (
                    <Sparkles size={13} className="text-fuchsia-200" aria-hidden="true" />
                  )}
                  {item.label}
                </div>
                <p className="mt-1 text-[10px] leading-4 text-cv-subtext">{item.description}</p>
              </li>
            );
          })}
        </ul>
        <p className="mt-2 text-[10px] text-cv-subtext">
          Always free: local drives, playback, the unified library and streaming on your home network.
        </p>

        {trialAvailable && (
          <button
            type="button"
            onClick={() => void startFreeTrial()}
            disabled={busy !== null}
            className="cv-btn cv-btn-primary mt-4 w-full justify-center"
            data-testid="cinavault-start-trial"
          >
            <Sparkles size={14} aria-hidden="true" />
            {busy === "trial" ? "Starting trial…" : `Start free ${PLUS_TRIAL_DAYS}-day trial`}
          </button>
        )}

        {!isPlus && (
          <div className="mt-4">
            <button
              type="button"
              onClick={() => void startCheckout()}
              disabled={!checkoutUrl || busy !== null}
              className="cv-btn cv-btn-primary w-full justify-center text-sm disabled:cursor-not-allowed disabled:opacity-50"
            >
              {busy === "checkout" ? (
                <Loader2 size={15} className="animate-spin" />
              ) : (
                <ExternalLink size={15} />
              )}
              Upgrade to {PLUS_PLAN_NAME} · {price}
            </button>
            {!checkoutUrl && (
              <p className="mt-2 text-[10px] text-cv-subtext">
                Online checkout isn't configured in this build. If you already
                have a {PLUS_PLAN_NAME} license key, activate it below.
              </p>
            )}
          </div>
        )}

        <div className="mt-4 border-t border-white/10 pt-4">
          <label className="section-label flex items-center gap-1.5" htmlFor={licenseInputId}>
            <KeyRound size={12} aria-hidden="true" /> License key
          </label>
          <div className="mt-1 flex flex-col gap-2 sm:flex-row">
            <input
              id={licenseInputId}
              type="password"
              autoComplete="off"
              spellCheck={false}
              value={token}
              onChange={(event) => setToken(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") void activateToken();
              }}
              placeholder="CVL1.…"
              className="cv-input min-w-0 flex-1 font-mono text-xs"
            />
            <button
              type="button"
              onClick={() => void activateToken()}
              disabled={busy !== null}
              className="cv-btn cv-btn-secondary justify-center text-xs disabled:opacity-50"
            >
              {busy === "activate" ? <Loader2 size={13} className="animate-spin" /> : <KeyRound size={13} />}
              {busy === "activate" ? "Activating…" : "Activate"}
            </button>
          </div>
          {entitlements && !entitlements.licensing_configured && (
            <p className="mt-2 text-[10px] text-cv-subtext">
              License verification isn't configured in this build, so keys can't be activated yet.
            </p>
          )}
        </div>

        {notice && (
          <p
            role="status"
            className={`mt-3 rounded-lg px-3 py-2 text-xs ${
              notice.kind === "ok"
                ? "border border-emerald-300/30 bg-emerald-400/10 text-emerald-100"
                : "border border-red-300/30 bg-red-500/10 text-red-100"
            }`}
          >
            {notice.text}
          </p>
        )}
      </div>
    </motion.div>
  );
}
