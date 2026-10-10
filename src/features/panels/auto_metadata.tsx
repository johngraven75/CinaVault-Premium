// Auto Metadata Fetch: what the background work after the last scan did.
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import type { FeaturePanel } from "./types";
import { describePostScanRun, type PostScanRun } from "../library/libraryAutomation.ts";

interface PostScanStatus {
  running: boolean;
  queued: number;
  lastRun: PostScanRun | null;
}

const AutoMetadataPanel: FeaturePanel = () => {
  const [status, setStatus] = useState<PostScanStatus | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    const load = () => invoke<PostScanStatus>("post_scan_status").then(setStatus).catch((e) => setError(String(e)));
    void load();
    const timer = window.setInterval(load, 4000);
    return () => window.clearInterval(timer);
  }, []);

  return (
    <div className="space-y-1 text-xs">
      <div className="text-cv-subtext">
        New titles from a scan are looked up in the background (keyless providers, then TMDB/OMDb when keys are
        stored). Progress and Stop use the metadata task bar. Off: scans only add files.
      </div>
      {status?.running && <div>Working on new titles{status.queued ? ` (${status.queued} queued)` : ""}...</div>}
      {describePostScanRun(status?.lastRun).map((line) => (
        <div key={line}>{line}</div>
      ))}
      {error && <div className="text-cv-subtext">{error}</div>}
    </div>
  );
};

export default AutoMetadataPanel;
