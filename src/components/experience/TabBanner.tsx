// CinaVault Premium — Reusable "live art" banner for tab headers.
//
// Every tab was getting its own hand-rolled hero block (or none at all —
// 13 of 14 tabs had no banner). This centralizes the pattern so every tab
// can adopt it with a few props instead of another one-off implementation,
// and so a single fix (motion, contrast, a11y) lands everywhere at once.
//
// Respects prefers-reduced-motion the same way Header.tsx and
// ExperienceBackdrop.tsx already do in this codebase: useReducedMotion()
// from framer-motion, animations skipped rather than merely shortened.
import type { JSX, ReactNode } from "react";
import { motion, useReducedMotion } from "framer-motion";
import type { LucideIcon } from "lucide-react";

export interface TabBannerProps {
  icon: LucideIcon;
  eyebrow: string;
  title: string;
  subtitle: string;
  /** Tailwind gradient stop classes, e.g. "from-cyan-400/20 to-fuchsia-400/10". */
  accent: string;
  /** Tailwind text/icon color class for the icon chip, e.g. "text-cyan-100". */
  accentText: string;
  children?: ReactNode;
}

export default function TabBanner({
  icon: Icon,
  eyebrow,
  title,
  subtitle,
  accent,
  accentText,
  children,
}: TabBannerProps): JSX.Element {
  const reduceMotion = useReducedMotion();

  return (
    <div
      className={`relative overflow-hidden rounded-[26px] border border-white/10 bg-[linear-gradient(120deg,rgba(255,255,255,0.05),rgba(255,255,255,0.01))] p-6`}
      role="banner"
    >
      <div
        aria-hidden="true"
        className={`pointer-events-none absolute inset-0 bg-gradient-to-br ${accent} opacity-70`}
      />
      <motion.div
        aria-hidden="true"
        className="pointer-events-none absolute -right-16 -top-16 h-56 w-56 rounded-full bg-white/10 blur-3xl"
        animate={
          reduceMotion
            ? undefined
            : { scale: [1, 1.15, 1], opacity: [0.5, 0.8, 0.5] }
        }
        transition={{ duration: 6, repeat: Infinity, ease: "easeInOut" }}
      />
      <motion.div
        aria-hidden="true"
        className="pointer-events-none absolute -bottom-20 left-12 h-48 w-48 rounded-full bg-black/20 blur-3xl"
        animate={
          reduceMotion
            ? undefined
            : { scale: [1, 1.2, 1], opacity: [0.4, 0.65, 0.4] }
        }
        transition={{ duration: 7, repeat: Infinity, ease: "easeInOut", delay: 0.8 }}
      />

      <div className="relative z-10 flex flex-wrap items-center justify-between gap-4">
        <div className="flex items-center gap-4">
          <span
            className={`grid h-14 w-14 shrink-0 place-items-center rounded-2xl border border-white/15 bg-black/25 shadow-[0_0_28px_rgba(0,0,0,0.25)] ${accentText}`}
          >
            <Icon size={26} aria-hidden="true" />
          </span>
          <div>
            <div className="text-[11px] font-bold uppercase tracking-[0.34em] text-white/70">
              {eyebrow}
            </div>
            <h2 className="text-2xl font-black tracking-tight text-white">
              {title}
            </h2>
            <p className="max-w-xl text-sm text-white/70">{subtitle}</p>
          </div>
        </div>
        {children}
      </div>
    </div>
  );
}
