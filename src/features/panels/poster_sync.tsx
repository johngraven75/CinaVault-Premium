// Cloud Poster Sync: the shared folder, adult opt-in and "Sync now".
import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import type { FeaturePanel } from "./types";
import { describePosterSyncRun, type PosterSyncRun } from "../library/libraryAutomation.ts";

interface PosterSyncConfig {
  folder?: string;
  includeAdult?: boolean;
}

interface PosterSyncStatus {
  folder: string | null;
  folderAvailable: boolean;
  lastRun: PosterSyncRun | null;
}

const PosterSyncPanel: FeaturePanel<PosterSyncConfig> = ({ enabled, config, setConfig }) => {
  const [status, setStatus] = useState<PosterSyncStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");

  const refresh = useCallback(() => {
    invoke<PosterSyncStatus>("poster_sync_status").then(setStatus).catch((error) => setMessage(String(error)));
  }, []);
  useEffect(refresh, [refresh, config.folder]);

  const choose = async () => {
    const picked = await open({ directory: true, multiple: false, title: "Folder to share artwork through" });
    if (typeof picked === "string" && picked) await setConfig({ folder: picked });
  };

  const syncNow = async () => {
    setBusy(true);
    setMessage("");
    try {
      const run = await invoke<PosterSyncRun>("poster_sync_now");
      setMessage(describePosterSyncRun(run));
      refresh();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-2 text-xs">
      <div className="text-cv-subtext">
        Pick a folder that your other PCs also see (OneDrive, Dropbox, a NAS share). Each title gets a
        subfolder with poster and backdrop; scans fill titles without artwork from it.
      </div>
      <div className="flex items-center gap-1">
        <input className="cv-input flex-1 text-xs" value={config.folder ?? ""} readOnly placeholder="No folder chosen" />
        <button type="button" className="cv-btn cv-btn-secondary text-[10px] px-2" onClick={() => void choose()}>
          Choose...
        </button>
      </div>
      {config.folder && status && !status.folderAvailable && (
        <div className="text-amber-400">The folder is not reachable right now.</div>
      )}
      <label className="flex items-center gap-2">
        <input
          type="checkbox"
          checked={Boolean(config.includeAdult)}
          onChange={(event) => void setConfig({ includeAdult: event.target.checked })}
        />
        Include adult titles
      </label>
      <div className="flex items-center gap-2">
        <button
          type="button"
          className="cv-btn cv-btn-secondary text-[10px] px-2 py-1"
          disabled={!enabled || !config.folder || busy}
          onClick={() => void syncNow()}
        >
          {busy ? "Syncing..." : "Sync now"}
        </button>
        {!enabled && <span className="text-cv-subtext">Turn the switch on to sync.</span>}
      </div>
      <div className="text-cv-subtext" role="status">
        {message || describePosterSyncRun(status?.lastRun)}
      </div>
    </div>
  );
};

export default PosterSyncPanel;
