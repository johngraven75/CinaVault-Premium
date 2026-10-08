// Per-stream cap for media the server sends to other devices.
import { useState } from "react";

import { effectiveMbps, mbpsHint, parseMbps } from "../serverAdminLogic.ts";
import { useAppStore } from "../../store/appStore";
import type { FeaturePanel } from "./types";

const BandwidthPanel: FeaturePanel<{ mbps?: number }> = ({ config, setConfig }) => {
  const legacy = useAppStore((state) => state.settings.remote_upload_limit_mbps);
  const mbps = effectiveMbps(config, legacy);
  const [text, setText] = useState(String(mbps));
  const [error, setError] = useState<string | null>(null);

  const save = () => {
    const value = parseMbps(text);
    if (value === null) return setError("Enter a number from 0.5 to 10000");
    setError(null);
    setText(String(value));
    if (value !== config.mbps) void setConfig({ mbps: value });
  };

  return (
    <div className="space-y-1.5">
      <label className="flex items-center gap-2 text-xs">
        Each stream at most
        <input
          className="cv-input w-20 text-xs"
          inputMode="decimal"
          value={text}
          onChange={(event) => setText(event.target.value)}
          onBlur={save}
          onKeyDown={(event) => event.key === "Enter" && save()}
          aria-label="Megabits per second"
        />
        Mbps <span className="text-[10px] text-cv-subtext">({mbpsHint(parseMbps(text) ?? mbps)})</span>
      </label>
      {error && <div className="text-[11px] text-rose-300">{error}</div>}
      <p className="text-[10px] text-cv-subtext">
        Applies to streams and transcodes sent to LAN, remote and relayed clients. Playback on this computer is never limited.
      </p>
    </div>
  );
};

export default BandwidthPanel;
