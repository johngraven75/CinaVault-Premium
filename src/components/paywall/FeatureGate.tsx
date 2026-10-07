// Wraps Plus-only UI. Unlocked (or entitlement state unknown) → children.
// Locked → a compact lock card with an upgrade call to action that opens
// the global CinaVault Plus panel.
import type { JSX, ReactNode } from "react";
import { motion, useReducedMotion } from "framer-motion";
import { Crown, Lock } from "lucide-react";
import {
  PLUS_PLAN_NAME,
  plusFeatureInfo,
  useEntitlementsStore,
  useFeatureUnlocked,
  type PlusFeature,
} from "../../services/entitlements";

export interface FeatureGateProps {
  feature: PlusFeature;
  children: ReactNode;
  /** Optional heading for the locked card (defaults to the feature label). */
  title?: string;
  /** When false the gate is bypassed (e.g. only gate the "Adult" group). */
  when?: boolean;
  /** "compact" renders a single-row lock strip for small sections. */
  variant?: "panel" | "compact";
}

export default function FeatureGate({
  feature,
  children,
  title,
  when = true,
  variant = "panel",
}: FeatureGateProps): JSX.Element {
  const unlocked = useFeatureUnlocked(feature);
  const showPaywall = useEntitlementsStore((state) => state.showPaywall);
  const reduceMotion = useReducedMotion();

  if (!when || unlocked) return <>{children}</>;

  const info = plusFeatureInfo(feature);
  const heading = title || info.label;
  const upgrade = () => showPaywall(feature);

  if (variant === "compact") {
    return (
      <div
        className="cv-feature-gate flex flex-wrap items-center justify-between gap-3 rounded-xl border border-fuchsia-300/25 bg-fuchsia-500/[0.07] px-3 py-2.5"
        data-plus-feature={feature}
      >
        <div className="flex min-w-0 items-center gap-2 text-xs text-cv-text">
          <Lock size={13} className="shrink-0 text-fuchsia-200" aria-hidden="true" />
          <span className="truncate">
            <strong>{heading}</strong> is part of {PLUS_PLAN_NAME}
          </span>
        </div>
        <button type="button" onClick={upgrade} className="cv-btn cv-btn-primary px-3 py-1 text-[11px]">
          <Crown size={12} aria-hidden="true" /> Upgrade
        </button>
      </div>
    );
  }

  return (
    <motion.div
      className="cv-feature-gate relative overflow-hidden rounded-[22px] border border-fuchsia-300/25 bg-[linear-gradient(135deg,rgba(40,12,60,0.7),rgba(6,14,28,0.75))] p-6 text-center"
      data-plus-feature={feature}
      initial={reduceMotion ? false : { opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.28 }}
    >
      <div
        aria-hidden="true"
        className="pointer-events-none absolute left-1/2 top-0 h-40 w-72 -translate-x-1/2 rounded-full bg-fuchsia-500/20 blur-3xl"
      />
      <div className="relative z-10 mx-auto max-w-md">
        <motion.span
          className="mx-auto grid h-12 w-12 place-items-center rounded-2xl border border-fuchsia-200/40 bg-fuchsia-400/15 text-fuchsia-100"
          animate={reduceMotion ? undefined : { boxShadow: ["0 0 0px rgba(217,70,239,0)", "0 0 26px rgba(217,70,239,0.45)", "0 0 0px rgba(217,70,239,0)"] }}
          transition={{ duration: 3.2, repeat: Infinity, ease: "easeInOut" }}
        >
          <Lock size={20} aria-hidden="true" />
        </motion.span>
        <h3 className="mt-3 text-base font-black text-white">{heading}</h3>
        <p className="mt-1 text-xs leading-5 text-cv-subtext">{info.description}</p>
        <p className="mt-1 text-[11px] text-fuchsia-100/80">Included with {PLUS_PLAN_NAME}.</p>
        <button type="button" onClick={upgrade} className="cv-btn cv-btn-primary mx-auto mt-4 text-xs">
          <Crown size={13} aria-hidden="true" /> See {PLUS_PLAN_NAME}
        </button>
      </div>
    </motion.div>
  );
}
