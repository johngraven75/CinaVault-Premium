// Profiles: create, rename, recolour, restrict and delete.
import { useCallback, useEffect, useState } from "react";
import { Plus, Save, ShieldCheck, Trash2 } from "lucide-react";

import PinUnlock from "../../components/profiles/PinUnlock";
import {
  OWNER_PROFILE_ID,
  PROFILE_COLORS,
  isHexColor,
  isPinRequired,
  profileNameError,
  type Profile,
} from "../profileLogic.ts";
import { createProfile, deleteProfile, listProfiles, saveProfile } from "../../services/profiles";
import type { FeaturePanel } from "./types";

type Draft = Pick<Profile, "id" | "name" | "color" | "restricted">;

const UserProfilesPanel: FeaturePanel = ({ enabled }) => {
  const [drafts, setDrafts] = useState<Draft[]>([]);
  const [newName, setNewName] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [needPin, setNeedPin] = useState(false);

  const reload = useCallback(async () => {
    try {
      setDrafts((await listProfiles()).map(({ id, name, color, restricted }) => ({ id, name, color, restricted })));
    } catch (error) {
      setMessage(String(error));
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  const run = async (action: () => Promise<unknown>, done: string) => {
    try {
      await action();
      setMessage(done);
      setNeedPin(false);
      await reload();
    } catch (error) {
      setNeedPin(isPinRequired(error));
      setMessage(String(error));
    }
  };

  const update = (id: number, patch: Partial<Draft>) =>
    setDrafts((list) => list.map((draft) => (draft.id === id ? { ...draft, ...patch } : draft)));

  const add = () => {
    const problem = profileNameError(newName);
    if (problem) return setMessage(problem);
    const color = PROFILE_COLORS[drafts.length % PROFILE_COLORS.length];
    void run(async () => {
      await createProfile(newName.trim(), color, false);
      setNewName("");
    }, `Added ${newName.trim()}`);
  };

  return (
    <div className="space-y-2">
      {!enabled && (
        <p className="text-[11px] text-cv-subtext">
          Profiles are kept, but everyone watches as Owner until this switch is on.
        </p>
      )}
      {drafts.map((draft) => {
        const nameProblem = profileNameError(draft.name);
        const colorOk = isHexColor(draft.color);
        return (
          <div key={draft.id} className="flex flex-wrap items-center gap-1.5">
            <input
              type="color"
              aria-label={`${draft.name} colour`}
              value={colorOk && draft.color.length === 7 ? draft.color : "#38bdf8"}
              onChange={(event) => update(draft.id, { color: event.target.value })}
              className="h-7 w-8 cursor-pointer rounded border border-white/10 bg-transparent"
            />
            <input
              className="cv-input min-w-[8rem] flex-1 text-xs"
              aria-label="Profile name"
              value={draft.name}
              maxLength={40}
              onChange={(event) => update(draft.id, { name: event.target.value })}
            />
            <label
              className={`flex items-center gap-1 text-[11px] ${draft.id === OWNER_PROFILE_ID ? "opacity-40" : ""}`}
              title={draft.id === OWNER_PROFILE_ID ? "The Owner profile holds the parental PIN and cannot be restricted" : "Parental controls apply to this profile"}
            >
              <input
                type="checkbox"
                checked={draft.restricted}
                disabled={draft.id === OWNER_PROFILE_ID}
                onChange={(event) => update(draft.id, { restricted: event.target.checked })}
              />
              <ShieldCheck size={11} /> Restricted
            </label>
            <button
              type="button"
              className="cv-btn cv-btn-secondary px-2 py-1 text-xs"
              disabled={Boolean(nameProblem) || !colorOk}
              onClick={() => void run(() => saveProfile(draft), `Saved ${draft.name.trim()}`)}
              aria-label={`Save ${draft.name}`}
            >
              <Save size={12} />
            </button>
            {draft.id !== OWNER_PROFILE_ID && (
              <button
                type="button"
                className="cv-btn cv-btn-secondary px-2 py-1 text-xs"
                onClick={() => {
                  if (window.confirm(`Delete ${draft.name}? Its progress and watchlist are removed.`)) {
                    void run(() => deleteProfile(draft.id), `Deleted ${draft.name}`);
                  }
                }}
                aria-label={`Delete ${draft.name}`}
              >
                <Trash2 size={12} />
              </button>
            )}
          </div>
        );
      })}
      <form
        className="flex gap-1.5"
        onSubmit={(event) => {
          event.preventDefault();
          add();
        }}
      >
        <input
          className="cv-input flex-1 text-xs"
          placeholder="New profile name"
          maxLength={40}
          value={newName}
          onChange={(event) => setNewName(event.target.value)}
        />
        <button type="submit" className="cv-btn text-xs">
          <Plus size={12} /> Add
        </button>
      </form>
      {needPin && <PinUnlock reason="This change needs the parental PIN" onUnlocked={() => setMessage("Unlocked for 5 minutes. Try again.")} />}
      {message && <div className="text-[11px] text-cv-subtext" role="status">{message}</div>}
    </div>
  );
};

export default UserProfilesPanel;
