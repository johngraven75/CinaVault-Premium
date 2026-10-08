// Compact "enter the parental PIN" row. Unlocking opens a five-minute window
// in which protected changes (switch off, rule changes, lifting a restriction)
// go through.
import { useState } from "react";
import { Lock } from "lucide-react";

import { pinError, type ParentalStatus } from "../../features/profileLogic.ts";
import { unlockParental } from "../../services/profiles";

export default function PinUnlock({
  reason,
  onUnlocked,
}: {
  reason: string;
  onUnlocked: (status: ParentalStatus) => void;
}) {
  const [pin, setPin] = useState("");
  const [error, setError] = useState<string | null>(null);

  const submit = async () => {
    const problem = pinError(pin);
    if (problem) return setError(problem);
    try {
      const status = await unlockParental(pin.trim());
      setPin("");
      setError(null);
      onUnlocked(status);
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <form
      className="space-y-1 rounded-lg border border-amber-300/25 bg-amber-300/[0.06] p-2"
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      <div className="flex items-center gap-1 text-[11px] text-amber-100">
        <Lock size={11} /> {reason}
      </div>
      <div className="flex gap-1.5">
        <input
          className="cv-input flex-1 text-xs"
          type="password"
          inputMode="numeric"
          autoComplete="off"
          placeholder="Parental PIN"
          aria-label="Parental PIN"
          value={pin}
          onChange={(event) => setPin(event.target.value.replace(/\D/g, "").slice(0, 8))}
        />
        <button type="submit" className="cv-btn text-xs">Unlock</button>
      </div>
      {error && <div className="text-[11px] text-rose-300" role="alert">{error}</div>}
    </form>
  );
}
