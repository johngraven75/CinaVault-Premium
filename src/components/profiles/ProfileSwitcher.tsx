// Header profile switcher, shown while "Multiple User Profiles" is on.
// Leaving a restricted profile asks for the parental PIN (the back end checks it too).
import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Check, Lock, ShieldCheck } from "lucide-react";

import { useFeature } from "../../features/featureFlags";
import {
  pinError,
  profileInitials,
  switchNeedsPin,
  type ParentalStatus,
  type Profile,
} from "../../features/profileLogic.ts";
import { activeProfile, listProfiles, parentalStatus, switchProfile } from "../../services/profiles";
import { useAppStore } from "../../store/appStore";

export default function ProfileSwitcher() {
  const { enabled } = useFeature("user_profiles");
  const addStatusMessage = useAppStore((state) => state.addStatusMessage);
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [active, setActive] = useState<Profile | null>(null);
  const [parental, setParental] = useState<ParentalStatus | null>(null);
  const [open, setOpen] = useState(false);
  const [pinFor, setPinFor] = useState<Profile | null>(null);
  const [pin, setPin] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [anchor, setAnchor] = useState<{ top: number; right: number }>({ top: 80, right: 16 });
  const buttonRef = useRef<HTMLButtonElement | null>(null);

  const reload = useCallback(async () => {
    try {
      const [list, current, status] = await Promise.all([listProfiles(), activeProfile(), parentalStatus()]);
      setProfiles(list);
      setActive(current);
      setParental(status);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  useEffect(() => {
    if (!enabled) return;
    void reload();
    const onChange = () => void reload();
    window.addEventListener("cinavault:profile-changed", onChange);
    return () => window.removeEventListener("cinavault:profile-changed", onChange);
  }, [enabled, reload]);

  useEffect(() => {
    if (!open) return;
    const rect = buttonRef.current?.getBoundingClientRect();
    if (rect) setAnchor({ top: rect.bottom + 8, right: Math.max(8, window.innerWidth - rect.right) });
    const close = (event: KeyboardEvent) => event.key === "Escape" && setOpen(false);
    window.addEventListener("keydown", close);
    return () => window.removeEventListener("keydown", close);
  }, [open]);

  if (!enabled) return null;

  const choose = async (target: Profile, withPin?: string) => {
    if (target.id === active?.id) {
      setOpen(false);
      return;
    }
    if (!withPin && switchNeedsPin(active, target, parental)) {
      setPinFor(target);
      setPin("");
      setError(null);
      return;
    }
    try {
      const profile = await switchProfile(target.id, withPin);
      setActive(profile);
      setPinFor(null);
      setOpen(false);
      addStatusMessage(`Watching as ${profile.name}`);
      void reload();
    } catch (err) {
      setError(String(err));
    }
  };

  const submitPin = () => {
    const problem = pinError(pin);
    if (problem) return setError(problem);
    if (pinFor) void choose(pinFor, pin.trim());
  };

  const badge = (profile: Profile, size = "h-9 w-9 text-xs") => (
    <span
      className={`grid ${size} shrink-0 place-items-center rounded-full font-black text-black/80`}
      style={{ background: profile.color }}
      aria-hidden="true"
    >
      {profileInitials(profile.name)}
    </span>
  );

  return (
    <>
      <button
        ref={buttonRef}
        type="button"
        onClick={() => setOpen((value) => !value)}
        className="grid h-11 w-11 shrink-0 place-items-center rounded-[15px] border border-white/[0.08] bg-white/[0.035] outline-none transition hover:border-cyan-200/25 focus-visible:ring-2 focus-visible:ring-cyan-300/70"
        title={active ? `Profile: ${active.name}` : "Profiles"}
        aria-label={active ? `Switch profile, current ${active.name}` : "Switch profile"}
        aria-haspopup="menu"
        aria-expanded={open}
      >
        {active ? badge(active, "h-8 w-8 text-[11px]") : <span className="text-xs">…</span>}
      </button>
      {open &&
        createPortal(
          <>
          <div className="fixed inset-0 z-[79]" aria-hidden="true" onClick={() => setOpen(false)} />
          <div
            role="menu"
            className="fixed z-[80] w-64 rounded-2xl border border-cyan-200/18 bg-[rgba(4,7,19,0.98)] p-2 text-xs shadow-[0_28px_80px_rgba(0,0,0,0.62)]"
            style={{ top: anchor.top, right: anchor.right }}
          >
            <div className="px-2 pb-2 pt-1 text-[10px] font-black uppercase tracking-[0.15em] text-cyan-100">Who's watching</div>
            {profiles.map((profile) => (
              <button
                key={profile.id}
                type="button"
                role="menuitem"
                onClick={() => void choose(profile)}
                className="flex w-full items-center gap-2 rounded-xl px-2 py-1.5 text-left hover:bg-white/[0.06]"
              >
                {badge(profile, "h-7 w-7 text-[10px]")}
                <span className="flex-1 truncate text-slate-100">{profile.name}</span>
                {profile.restricted && <ShieldCheck size={13} className="text-amber-300" aria-label="Restricted" />}
                {profile.id === active?.id && <Check size={13} className="text-cyan-200" aria-label="Current" />}
              </button>
            ))}
            {pinFor && (
              <form
                className="mt-2 space-y-1.5 border-t border-white/10 px-2 pt-2"
                onSubmit={(event) => {
                  event.preventDefault();
                  submitPin();
                }}
              >
                <label className="flex items-center gap-1 text-[11px] text-slate-300" htmlFor="cv-profile-pin">
                  <Lock size={11} /> Parental PIN to switch to {pinFor.name}
                </label>
                <div className="flex gap-1.5">
                  <input
                    id="cv-profile-pin"
                    className="cv-input flex-1 text-xs"
                    type="password"
                    inputMode="numeric"
                    autoComplete="off"
                    autoFocus
                    value={pin}
                    onChange={(event) => setPin(event.target.value.replace(/\D/g, "").slice(0, 8))}
                  />
                  <button type="submit" className="cv-btn text-xs">Unlock</button>
                </div>
              </form>
            )}
            {error && <div className="mt-2 px-2 text-[11px] text-rose-300" role="alert">{error}</div>}
          </div>
          </>,
          document.body,
        )}
    </>
  );
}
