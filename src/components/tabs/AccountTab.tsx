// CinaVault Premium — Account & Plan tab: plan status, license, Plus
// features, and a link to remote-user administration (RemoteAccessTab).
import { useEffect, useState } from "react";
import type { JSX } from "react";
import { motion, useReducedMotion } from "framer-motion";
import {
  BadgeCheck,
  CalendarClock,
  CheckCircle2,
  Crown,
  Loader2,
  Lock,
  LogOut,
  Mail,
  RefreshCw,
  Router,
  Sparkles,
  UserCog,
  Wand2,
} from "lucide-react";
import TabBanner from "../experience/TabBanner";
import { IS_STORE_SAFE } from "../../config/edition";
import PaywallPanel from "../paywall/PaywallPanel";
import { openFirstRunSetup } from "../setup/FirstRunSetup";
import { useAppStore } from "../../store/appStore";
import {
  visiblePlusFeatures,
  PLUS_PLAN_NAME,
  isFeatureUnlocked,
  planLabel,
  useEntitlementsStore,
} from "../../services/entitlements";

function formatDate(value: string | null | undefined): string {
  if (!value) return "—";
  const parsed = Date.parse(value);
  return Number.isFinite(parsed) ? new Date(parsed).toLocaleDateString() : value;
}

export default function AccountTab(): JSX.Element {
  const reduceMotion = useReducedMotion();
  const setActiveTab = useAppStore((state) => state.setActiveTab);
  const addStatusMessage = useAppStore((state) => state.addStatusMessage);
  const entitlements = useEntitlementsStore((state) => state.entitlements);
  const status = useEntitlementsStore((state) => state.status);
  const refresh = useEntitlementsStore((state) => state.refresh);
  const deactivate = useEntitlementsStore((state) => state.deactivate);
  const [busy, setBusy] = useState<"refresh" | "deactivate" | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const isPlus = entitlements?.plan === "plus";
  const unavailable = status === "unavailable";

  const reload = async () => {
    setBusy("refresh");
    await refresh();
    setBusy(null);
  };

  const removeLicense = async () => {
    if (!window.confirm(`Remove the ${PLUS_PLAN_NAME} license from this computer?`)) return;
    setBusy("deactivate");
    setNotice(null);
    try {
      await deactivate();
      setNotice("License removed from this computer.");
      addStatusMessage(`${PLUS_PLAN_NAME} license deactivated`);
    } catch (error) {
      setNotice(`Couldn't deactivate: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="space-y-5">
      <TabBanner
        icon={UserCog}
        eyebrow="Account & Plan"
        title="Your CinaVault"
        subtitle="Plan status, license activation, and who can reach your server."
        accent="from-amber-300/28 to-fuchsia-500/12"
        accentText="text-amber-100"
      />

      <div className="grid gap-5 xl:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]">
        <div className="space-y-5">
          <motion.div
            className="glass-panel relative overflow-hidden p-5"
            initial={reduceMotion ? false : { opacity: 0, y: 12 }}
            animate={{ opacity: 1, y: 0 }}
          >
            <div className="flex items-start justify-between gap-3">
              <div>
                <div className="section-label">Current plan</div>
                <div className="mt-1 flex items-center gap-2 text-2xl font-black tracking-tight text-white">
                  {isPlus ? <Crown size={22} className="text-amber-200" /> : <Sparkles size={20} className="text-cyan-200" />}
                  {unavailable ? "CinaVault Premium" : planLabel(entitlements)}
                </div>
                {unavailable && (
                  <p className="mt-1 text-[11px] text-cv-subtext">
                    Plan details aren't available from this server build; all features are shown.
                  </p>
                )}
              </div>
              <button type="button" onClick={() => void reload()} disabled={busy !== null} className="cv-btn cv-btn-secondary h-9 w-9 px-0" aria-label="Refresh plan status">
                <RefreshCw size={14} className={busy === "refresh" || status === "loading" ? "animate-spin" : ""} />
              </button>
            </div>

            <dl className="mt-4 grid gap-2 text-xs">
              {entitlements?.plan === "trial" && (
                <div className="terminal-line flex items-center justify-between gap-3 rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2">
                  <dt className="flex items-center gap-2 text-cv-subtext"><CalendarClock size={13} /> Trial ends</dt>
                  <dd className="font-semibold text-cv-text">
                    {formatDate(entitlements.trial_ends_at)} · {entitlements.trial_days_left ?? 0} days left
                  </dd>
                </div>
              )}
              <div className="flex items-center justify-between gap-3 rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2">
                <dt className="flex items-center gap-2 text-cv-subtext"><Mail size={13} /> License email</dt>
                <dd className="truncate font-semibold text-cv-text">{entitlements?.license_email || "No license"}</dd>
              </div>
              <div className="flex items-center justify-between gap-3 rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2">
                <dt className="flex items-center gap-2 text-cv-subtext"><BadgeCheck size={13} /> License expires</dt>
                <dd className="font-semibold text-cv-text">{formatDate(entitlements?.license_expires_at)}</dd>
              </div>
            </dl>

            {isPlus && (
              <button type="button" onClick={() => void removeLicense()} disabled={busy !== null} className="cv-btn cv-btn-secondary mt-4 text-xs disabled:opacity-50">
                {busy === "deactivate" ? <Loader2 size={13} className="animate-spin" /> : <LogOut size={13} />} Deactivate on this computer
              </button>
            )}
            {notice && <p role="status" className="mt-3 text-xs text-cv-subtext">{notice}</p>}
          </motion.div>

          <div className="glass-panel p-5">
            <div className="section-label">Plus features</div>
            <ul className="mt-3 grid gap-2">
              {visiblePlusFeatures(IS_STORE_SAFE).map((feature) => {
                const unlocked = isFeatureUnlocked(entitlements, feature.id);
                return (
                  <li key={feature.id} className="flex items-center justify-between gap-3 rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2">
                    <span className="text-xs font-semibold text-cv-text">{feature.label}</span>
                    <span className={`flex items-center gap-1 text-[10px] font-bold uppercase tracking-[0.12em] ${unlocked ? "text-emerald-200" : "text-fuchsia-200"}`}>
                      {unlocked ? <CheckCircle2 size={12} /> : <Lock size={12} />} {unlocked ? "Unlocked" : "Plus"}
                    </span>
                  </li>
                );
              })}
            </ul>
          </div>

          <div className="glass-panel p-5">
            <div className="section-label">Server administration</div>
            <p className="mt-1 text-xs text-cv-subtext">
              Remote users, access keys and sessions are managed in Remote Access.
            </p>
            <div className="mt-3 flex flex-wrap gap-2">
              <button type="button" onClick={() => setActiveTab("remote")} className="cv-btn cv-btn-secondary text-xs">
                <Router size={13} /> Manage remote users
              </button>
              <button type="button" onClick={openFirstRunSetup} className="cv-btn cv-btn-secondary text-xs">
                <Wand2 size={13} /> Run setup wizard
              </button>
            </div>
          </div>
        </div>

        <div>
          {isPlus ? (
            <div className="glass-panel p-6 text-center">
              <Crown size={34} className="mx-auto text-amber-200" />
              <h3 className="mt-3 text-lg font-black text-white">{PLUS_PLAN_NAME} is active</h3>
              <p className="mt-1 text-xs text-cv-subtext">
                Every Plus feature is unlocked on this computer.
              </p>
            </div>
          ) : unavailable ? (
            // No entitlement command (older back end): features stay unlocked
            // and the back end decides, so don't present an upgrade offer.
            <div className="glass-panel p-6 text-center" data-testid="cinavault-plan-unavailable">
              <Sparkles size={30} className="mx-auto text-cyan-200" />
              <h3 className="mt-3 text-lg font-black text-white">Plan status unavailable</h3>
              <p className="mt-1 text-xs text-cv-subtext">
                This server doesn't report a plan, so no features are locked here.
              </p>
            </div>
          ) : (
            <PaywallPanel />
          )}
        </div>
      </div>
    </div>
  );
}
