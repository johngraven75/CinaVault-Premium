import { useEffect, useState } from "react";

import type { FeaturePanel } from "./types";
import { transcodeStatus, type TranscodeStatus } from "../../components/player/playerApi";

// Read-only: shows which encoder transcodes use on this PC.
const HwTranscodePanel: FeaturePanel = ({ enabled }) => {
  const [status, setStatus] = useState<TranscodeStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    transcodeStatus()
      .then((next) => {
        if (!cancelled) setStatus(next);
      })
      .catch((reason) => {
        if (!cancelled) setError(String(reason));
      });
    return () => {
      cancelled = true;
    };
  }, [enabled]);
  if (error) return <p className="text-red-300">Encoder check failed: {error}</p>;
  if (!status) return <p className="text-cv-text-dim">Checking encoders…</p>;
  if (!status.ffmpegAvailable) {
    return <p className="text-amber-300">ffmpeg is not installed, so nothing can be transcoded.</p>;
  }
  return (
    <div className="space-y-1">
      <p>
        Transcodes use: <span className="font-semibold">{status.encoder}</span>
      </p>
      <p className="text-cv-text-dim">
        Working GPU encoders: {status.usableHardware.length ? status.usableHardware.join(", ") : "none found"}. Bitrate
        follows Settings &gt; Performance &gt; Quality Control.
      </p>
    </div>
  );
};

export default HwTranscodePanel;
