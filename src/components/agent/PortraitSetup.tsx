// CinaVault Premium — lets the user give the AI agent their own face.
// They pick a photo, drag three markers onto their eyes and mouth, and save;
// the photo goes to app data through agent_avatar.rs. Removing it brings the
// hologram head back.
import { useRef, useState } from "react";
import type { PointerEvent as ReactPointerEvent } from "react";

import {
  PORTRAIT_IMAGE_TYPES,
  calibrationError,
  defaultCalibration,
  type PortraitCalibration,
} from "../../services/portraitAvatar";
import {
  clearAgentAvatar,
  readPortraitFile,
  saveAgentAvatar,
  setAgentAvatarCalibration,
  type AgentAvatar,
} from "../../services/agentAvatarStore";
import { errorText } from "../../services/agentBrain";

type MarkerKey = keyof PortraitCalibration;

const MARKERS: { key: MarkerKey; label: string }[] = [
  { key: "leftEye", label: "Left eye" },
  { key: "rightEye", label: "Right eye" },
  { key: "mouth", label: "Mouth" },
];

interface Draft {
  /** A newly picked photo, or null when only moving the markers. */
  photo: string | null;
  preview: string;
  calibration: PortraitCalibration;
}

interface PortraitSetupProps {
  avatar: AgentAvatar | null;
  onChange: (avatar: AgentAvatar | null) => void;
}

export default function PortraitSetup({ avatar, onChange }: PortraitSetupProps) {
  const [draft, setDraft] = useState<Draft | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  /** Width/height of the photo being marked, once it has loaded. */
  const [aspect, setAspect] = useState(1);
  const fileRef = useRef<HTMLInputElement | null>(null);
  const frameRef = useRef<HTMLDivElement | null>(null);
  const dragging = useRef<MarkerKey | null>(null);

  const pick = async (file: File | undefined) => {
    if (!file) return;
    setError(null);
    try {
      const photo = await readPortraitFile(file);
      setDraft({ photo, preview: photo, calibration: defaultCalibration() });
    } catch (problem) {
      setError(errorText(problem));
    }
  };

  const moveMarker = (event: ReactPointerEvent) => {
    const key = dragging.current;
    const frame = frameRef.current;
    if (!key || !frame || !draft) return;
    const box = frame.getBoundingClientRect();
    const x = Math.min(1, Math.max(0, (event.clientX - box.left) / box.width));
    const y = Math.min(1, Math.max(0, (event.clientY - box.top) / box.height));
    setDraft({ ...draft, calibration: { ...draft.calibration, [key]: { x, y } } });
  };

  const save = async () => {
    if (!draft) return;
    const problem = calibrationError(draft.calibration, aspect);
    if (problem) {
      setError(problem);
      return;
    }
    setSaving(true);
    setError(null);
    try {
      const saved = draft.photo
        ? await saveAgentAvatar(draft.photo, draft.calibration)
        : await setAgentAvatarCalibration(draft.calibration);
      onChange(saved);
      setDraft(null);
    } catch (problem) {
      setError(errorText(problem));
    } finally {
      setSaving(false);
    }
  };

  const remove = async () => {
    setError(null);
    try {
      await clearAgentAvatar();
      onChange(null);
    } catch (problem) {
      setError(errorText(problem));
    }
  };

  const fileInput = (
    <input
      ref={fileRef}
      type="file"
      accept={PORTRAIT_IMAGE_TYPES.join(",")}
      className="hidden"
      onChange={(event) => {
        void pick(event.target.files?.[0]);
        event.target.value = "";
      }}
    />
  );

  if (draft) {
    return (
      <div className="holo-portrait-setup">
        <p className="text-xs text-cv-subtext">Drag each marker onto the centre of your eyes and mouth.</p>
        <div
          ref={frameRef}
          className="holo-portrait-frame"
          onPointerMove={moveMarker}
          onPointerUp={() => (dragging.current = null)}
          onPointerCancel={() => (dragging.current = null)}
        >
          <img
            src={draft.preview}
            alt="Your photo"
            draggable={false}
            onLoad={(event) => setAspect(event.currentTarget.naturalWidth / Math.max(1, event.currentTarget.naturalHeight))}
          />
          {MARKERS.map(({ key, label }) => (
            <button
              key={key}
              type="button"
              className={`holo-portrait-marker is-${key}`}
              style={{ left: `${draft.calibration[key].x * 100}%`, top: `${draft.calibration[key].y * 100}%` }}
              aria-label={`${label} marker`}
              title={label}
              onPointerDown={(event) => {
                dragging.current = key;
                frameRef.current?.setPointerCapture(event.pointerId);
              }}
            />
          ))}
        </div>
        <div className="flex flex-wrap gap-2">
          <button type="button" className="holo-agent-chip" disabled={saving} onClick={() => void save()}>
            {saving ? "Saving…" : "Save face"}
          </button>
          <button type="button" className="holo-agent-chip" disabled={saving} onClick={() => setDraft(null)}>
            Cancel
          </button>
        </div>
        {error && <p className="text-xs text-red-300" role="alert">{error}</p>}
      </div>
    );
  }

  return (
    <div className="holo-portrait-setup">
      {fileInput}
      <div className="flex items-center gap-2">
        {avatar && <img className="holo-agent-thumb" src={avatar.dataUrl} alt="Agent face" />}
        <p className="flex-1 text-xs text-cv-subtext">
          {avatar
            ? "The agent uses your photo as its face. It stays on this computer."
            : "Give the agent your face: pick a clear, front-facing photo. It stays on this computer."}
        </p>
      </div>
      <div className="flex flex-wrap gap-2">
        <button type="button" className="holo-agent-chip" onClick={() => fileRef.current?.click()}>
          {avatar ? "Change photo" : "Use my photo"}
        </button>
        {avatar && (
          <>
            <button
              type="button"
              className="holo-agent-chip"
              onClick={() => setDraft({ photo: null, preview: avatar.dataUrl, calibration: avatar.calibration })}
            >
              Adjust markers
            </button>
            <button type="button" className="holo-agent-chip" onClick={() => void remove()}>
              Remove photo
            </button>
          </>
        )}
      </div>
      {error && <p className="text-xs text-red-300" role="alert">{error}</p>}
    </div>
  );
}
