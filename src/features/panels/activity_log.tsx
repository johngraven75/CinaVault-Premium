// Recent activity: playback, scans, profiles, server and remote sign-ins.
import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw, Trash2 } from "lucide-react";

import { activityGroup, relativeTime, type ActivityEntry } from "../serverAdminLogic.ts";
import type { FeaturePanel } from "./types";

const ActivityLogPanel: FeaturePanel = ({ enabled }) => {
  const [entries, setEntries] = useState<ActivityEntry[]>([]);
  const [group, setGroup] = useState("all");
  const [message, setMessage] = useState<string | null>(null);

  const reload = useCallback(async () => {
    try {
      setEntries(await invoke<ActivityEntry[]>("activity_log_list", { limit: 200 }));
    } catch (error) {
      setMessage(String(error));
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  const groups = useMemo(() => [...new Set(entries.map((entry) => activityGroup(entry.kind)))].sort(), [entries]);
  const shown = group === "all" ? entries : entries.filter((entry) => activityGroup(entry.kind) === group);

  const clear = async () => {
    if (!window.confirm("Clear the whole activity log?")) return;
    try {
      await invoke("activity_log_clear");
      setEntries([]);
      setMessage("Log cleared");
    } catch (error) {
      setMessage(String(error));
    }
  };

  return (
    <div className="space-y-2">
      {!enabled && <p className="text-[11px] text-cv-subtext">Logging is paused; older entries stay until cleared.</p>}
      <div className="flex items-center gap-1.5">
        <select className="cv-select text-xs" value={group} onChange={(event) => setGroup(event.target.value)} aria-label="Filter by kind">
          <option value="all">All kinds</option>
          {groups.map((name) => (
            <option key={name} value={name}>
              {name}
            </option>
          ))}
        </select>
        <button type="button" className="cv-btn cv-btn-secondary text-xs" onClick={() => void reload()}>
          <RefreshCw size={12} /> Refresh
        </button>
        <button type="button" className="cv-btn cv-btn-secondary text-xs" onClick={() => void clear()} disabled={entries.length === 0}>
          <Trash2 size={12} /> Clear
        </button>
      </div>
      <ul className="max-h-60 space-y-0.5 overflow-y-auto pr-1">
        {shown.length === 0 && <li className="text-[11px] text-cv-subtext">Nothing logged yet.</li>}
        {shown.map((entry) => (
          <li key={entry.id} className="flex items-baseline gap-2 rounded px-1.5 py-1 hover:bg-white/[0.03]">
            <time className="w-16 shrink-0 text-[10px] text-cv-subtext" dateTime={entry.at} title={new Date(entry.at).toLocaleString()}>
              {relativeTime(entry.at)}
            </time>
            <span className="shrink-0 font-mono text-[10px] text-cv-accent">{entry.kind}</span>
            <span className="truncate text-xs">{entry.title}</span>
          </li>
        ))}
      </ul>
      {message && <div className="text-[11px] text-cv-subtext" role="status">{message}</div>}
    </div>
  );
};

export default ActivityLogPanel;
