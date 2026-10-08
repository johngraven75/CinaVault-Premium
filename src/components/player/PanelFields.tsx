// Small inputs shared by the player switches' settings panels.
import { useEffect, useState } from "react";
import type { FC } from "react";

interface NumberFieldProps {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  unit?: string;
  onCommit: (value: number) => void;
}

/** Number input that saves a clamped value on blur or Enter. */
export const NumberField: FC<NumberFieldProps> = ({ label, value, min, max, step = 1, unit, onCommit }) => {
  const [draft, setDraft] = useState(String(value));
  useEffect(() => setDraft(String(value)), [value]);
  const commit = () => {
    const n = Number(draft);
    const next = draft.trim() !== "" && Number.isFinite(n) ? Math.min(max, Math.max(min, n)) : value;
    setDraft(String(next));
    if (next !== value) onCommit(next);
  };
  return (
    <label className="flex items-center justify-between gap-3">
      <span className="text-cv-text-dim">{label}</span>
      <span className="flex items-center gap-1">
        <input
          type="number"
          className="cv-input w-20 text-xs"
          min={min}
          max={max}
          step={step}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onBlur={commit}
          onKeyDown={(event) => {
            if (event.key === "Enter") commit();
          }}
        />
        {unit && <span className="text-cv-text-dim">{unit}</span>}
      </span>
    </label>
  );
};

interface ToggleFieldProps {
  label: string;
  checked: boolean;
  onChange: (value: boolean) => void;
}

export const ToggleField: FC<ToggleFieldProps> = ({ label, checked, onChange }) => (
  <div className="flex items-center justify-between gap-3">
    <span className="text-cv-text-dim">{label}</span>
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      className={`cv-toggle ${checked ? "active" : ""}`}
      onClick={() => onChange(!checked)}
    />
  </div>
);
