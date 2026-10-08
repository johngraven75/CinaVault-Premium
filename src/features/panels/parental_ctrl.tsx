// Parental controls: PIN, highest rating, blocked genres, adult titles.
import { useCallback, useEffect, useRef, useState } from "react";
import { KeyRound, Lock, RefreshCw } from "lucide-react";

import PinUnlock from "../../components/profiles/PinUnlock";
import {
  isPinRequired,
  parentalConfig,
  parseGenres,
  pinError,
  type ParentalConfig,
  type ParentalStatus,
} from "../profileLogic.ts";
import {
  announceLibraryChange,
  lockParental,
  parentalStatus,
  refreshRatings,
  setParentalPin,
} from "../../services/profiles";
import type { FeaturePanel } from "./types";

const ParentalPanel: FeaturePanel<Partial<ParentalConfig>> = ({ enabled, config, setConfig }) => {
  const rules = parentalConfig(config);
  const [status, setStatus] = useState<ParentalStatus | null>(null);
  const [genresText, setGenresText] = useState(rules.blockedGenres.join(", "));
  const [newPin, setNewPin] = useState("");
  const [currentPin, setCurrentPin] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [needPin, setNeedPin] = useState(false);
  const [busy, setBusy] = useState(false);
  const firstRender = useRef(true);

  const reload = useCallback(async () => {
    try {
      setStatus(await parentalStatus());
    } catch (error) {
      setMessage(String(error));
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  // Turning the switch on or off changes what restricted profiles see.
  useEffect(() => {
    if (firstRender.current) {
      firstRender.current = false;
      return;
    }
    announceLibraryChange("parental-switch");
    void reload();
  }, [enabled, reload]);

  const save = async (next: Partial<ParentalConfig>) => {
    try {
      await setConfig({ ...rules, ...next });
      setNeedPin(false);
      setMessage("Saved");
      announceLibraryChange("parental-rules");
    } catch (error) {
      setNeedPin(isPinRequired(error));
      setMessage(String(error));
    }
  };

  const savePin = async () => {
    const problem = pinError(newPin);
    if (problem) return setMessage(problem);
    try {
      setStatus(await setParentalPin(newPin.trim(), currentPin.trim()));
      setNewPin("");
      setCurrentPin("");
      setMessage("PIN saved");
    } catch (error) {
      setMessage(String(error));
    }
  };

  const fillRatings = async (force: boolean) => {
    setBusy(true);
    try {
      const result = await refreshRatings(force);
      setMessage(
        `Checked ${result.checked}: ${result.fromNfo} from NFO files, ${result.fromTmdb} from TMDB, ${result.unrated} without a rating`,
      );
      announceLibraryChange("ratings-refreshed");
      await reload();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  };

  const choices = status?.maxRatingChoices ?? ["G", "PG", "PG-13", "R", "NC-17"];

  return (
    <div className="space-y-2.5">
      {status && !status.hasPin && (
        <p className="text-[11px] text-amber-200">
          Set a PIN so a restricted profile cannot switch away or turn these rules off.
        </p>
      )}
      <div className="flex flex-wrap items-end gap-1.5">
        <KeyRound size={12} className="mb-2 text-cv-accent" />
        {status?.hasPin && !status.unlocked && (
          <input
            className="cv-input w-28 text-xs"
            type="password"
            inputMode="numeric"
            placeholder="Current PIN"
            aria-label="Current PIN"
            value={currentPin}
            onChange={(event) => setCurrentPin(event.target.value.replace(/\D/g, "").slice(0, 8))}
          />
        )}
        <input
          className="cv-input w-28 text-xs"
          type="password"
          inputMode="numeric"
          placeholder={status?.hasPin ? "New PIN" : "PIN (4-8 digits)"}
          aria-label="New PIN"
          value={newPin}
          onChange={(event) => setNewPin(event.target.value.replace(/\D/g, "").slice(0, 8))}
        />
        <button type="button" className="cv-btn cv-btn-secondary text-xs" onClick={() => void savePin()}>
          {status?.hasPin ? "Change PIN" : "Set PIN"}
        </button>
        {status?.unlocked && (
          <button
            type="button"
            className="cv-btn cv-btn-secondary text-xs"
            onClick={() => void lockParental().then(setStatus)}
          >
            <Lock size={12} /> Lock
          </button>
        )}
      </div>

      <div className="grid grid-cols-2 gap-2">
        <label className="space-y-1">
          <span className="text-[11px] text-cv-subtext">Highest rating</span>
          <select
            className="cv-select w-full text-xs"
            value={rules.maxRating}
            onChange={(event) => void save({ maxRating: event.target.value })}
          >
            <option value="">Any rating</option>
            {choices.map((choice) => (
              <option key={choice} value={choice}>
                {choice}
              </option>
            ))}
          </select>
        </label>
        <label className="space-y-1">
          <span className="text-[11px] text-cv-subtext">Rating country</span>
          <input
            className="cv-input w-full text-xs uppercase"
            maxLength={2}
            value={rules.region}
            onChange={(event) => {
              const region = event.target.value.replace(/[^a-z]/gi, "").toUpperCase();
              if (region.length === 2) void save({ region });
            }}
          />
        </label>
      </div>
      <label className="block space-y-1">
        <span className="text-[11px] text-cv-subtext">Blocked genres (comma separated)</span>
        <input
          className="cv-input w-full text-xs"
          value={genresText}
          placeholder="Horror, Thriller"
          onChange={(event) => setGenresText(event.target.value)}
          onBlur={() => {
            const blockedGenres = parseGenres(genresText);
            setGenresText(blockedGenres.join(", "));
            if (blockedGenres.join("|") !== rules.blockedGenres.join("|")) void save({ blockedGenres });
          }}
        />
      </label>
      <label className="flex items-center gap-2 text-xs">
        <input type="checkbox" checked={rules.blockAdult} onChange={(event) => void save({ blockAdult: event.target.checked })} />
        Hide adult titles
      </label>
      <label className="flex items-center gap-2 text-xs">
        <input type="checkbox" checked={rules.blockUnrated} onChange={(event) => void save({ blockUnrated: event.target.checked })} />
        Hide titles without a known rating (when a highest rating is set)
      </label>

      <div className="flex flex-wrap items-center gap-2 text-[11px] text-cv-subtext">
        <span>
          Ratings known for {status?.ratedItems ?? 0} titles, {status?.unratedItems ?? 0} unknown.
        </span>
        <button type="button" className="cv-btn cv-btn-secondary text-xs" disabled={busy} onClick={() => void fillRatings(false)}>
          <RefreshCw size={12} className={busy ? "animate-spin" : ""} /> Look up ratings
        </button>
        <button type="button" className="cv-btn cv-btn-secondary text-xs" disabled={busy} onClick={() => void fillRatings(true)}>
          Re-check all
        </button>
      </div>
      <p className="text-[10px] text-cv-subtext">
        Ratings come from NFO files (&lt;mpaa&gt;) and TMDB for titles with a TMDB id. Mark profiles as restricted under Multiple User Profiles.
        {enabled && status?.hasPin && !status.unlocked && " Turning this switch off or changing rules needs the PIN."}
      </p>
      {(needPin || (enabled && status?.hasPin && !status.unlocked)) && (
        <PinUnlock
          reason="Unlock to change rules or turn parental controls off"
          onUnlocked={(next) => {
            setStatus(next);
            setNeedPin(false);
            setMessage("Unlocked for 5 minutes");
          }}
        />
      )}
      {message && <div className="text-[11px] text-cv-subtext" role="status">{message}</div>}
    </div>
  );
};

export default ParentalPanel;
