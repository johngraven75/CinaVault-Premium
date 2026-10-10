// CinaVault Premium — Duplicate finder (find_duplicates + quarantine)
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Copy, Search, ShieldAlert } from "lucide-react";
import { useAppStore } from "../../store/appStore";

import {
  DUPLICATE_MODES,
  formatBytes,
  keeperId,
  type DuplicateFile,
  type DuplicateScanResult,
} from "../../utils/duplicates";

export default function DuplicateFinder() {
  const { addStatusMessage } = useAppStore();
  const [mode, setMode] = useState<string>("name_size");
  const [result, setResult] = useState<DuplicateScanResult | null>(null);
  const [searching, setSearching] = useState(false);
  const [busyId, setBusyId] = useState<number | null>(null);
  const [quarantined, setQuarantined] = useState<Set<number>>(new Set());

  const search = async () => {
    setSearching(true);
    try {
      const found = await invoke<DuplicateScanResult>("find_duplicates", { mode });
      setResult(found);
      setQuarantined(new Set());
      addStatusMessage(
        `Duplicate check: ${found.groups.length} groups across ${found.scanned_files} files, ${formatBytes(found.total_wasted_bytes)} reclaimable`,
      );
    } catch (error) {
      addStatusMessage(`Duplicate check failed: ${error}`);
    } finally {
      setSearching(false);
    }
  };

  const quarantine = async (file: DuplicateFile) => {
    setBusyId(file.id);
    try {
      const moved = await invoke<string>("quarantine", { itemId: file.id });
      setQuarantined((prev) => new Set(prev).add(file.id));
      addStatusMessage(`Moved to quarantine: ${moved}`);
    } catch (error) {
      addStatusMessage(`Could not quarantine ${file.name}: ${error}`);
    } finally {
      setBusyId(null);
    }
  };

  return (
    <div className="glass-panel p-5">
      <h3 className="text-sm font-bold mb-1 flex items-center gap-2">
        <Copy size={16} className="text-cv-accent" /> Duplicate Finder
      </h3>
      <p className="text-xs text-cv-subtext mb-4">
        Finds copies of the same file across all sources. Quarantine moves a copy into CinaVault's quarantine folder
        instead of deleting it, so it can be restored.
      </p>
      <div className="flex flex-wrap items-center gap-2 mb-4">
        <select value={mode} onChange={(e) => setMode(e.target.value)} className="cv-select" aria-label="Match duplicates by">
          {DUPLICATE_MODES.map((m) => (
            <option key={m.id} value={m.id}>{m.label}</option>
          ))}
        </select>
        <button type="button" onClick={() => void search()} disabled={searching} className="cv-btn cv-btn-primary disabled:opacity-50">
          <Search size={14} className={searching ? "animate-spin" : ""} />
          {searching ? "Checking library..." : "Find duplicates"}
        </button>
        {result && (
          <span className="text-xs text-cv-subtext">
            {result.groups.length} groups · {formatBytes(result.total_wasted_bytes)} reclaimable
          </span>
        )}
      </div>
      {result && result.groups.length === 0 && (
        <p className="text-sm text-cv-subtext">No duplicates found in {result.scanned_files} files.</p>
      )}
      {result && result.groups.length > 0 && (
        <div className="space-y-2 max-h-[420px] overflow-y-auto pr-1">
          {result.groups.map((group) => {
            const keep = keeperId(group);
            return (
              <div key={group.key} className="glass-panel-2 rounded-lg p-3">
                <div className="flex items-center justify-between text-xs mb-2">
                  <span className="font-semibold truncate">{group.files[0]?.name || group.key}</span>
                  <span className="text-cv-subtext shrink-0 ml-2">{group.count} copies · {formatBytes(group.total_size)}</span>
                </div>
                <div className="space-y-1">
                  {group.files.map((file) => (
                    <div key={file.id} className="flex items-center gap-2 text-xs">
                      <span className="flex-1 min-w-0 truncate text-cv-subtext" title={file.path}>{file.path}</span>
                      <span className="shrink-0 text-cv-subtext">{formatBytes(file.size)}</span>
                      {file.id === keep ? (
                        <span className="shrink-0 rounded px-2 py-0.5 bg-white/5 text-cv-subtext">Keep</span>
                      ) : quarantined.has(file.id) ? (
                        <span className="shrink-0 rounded px-2 py-0.5 bg-white/5 text-cv-subtext">Quarantined</span>
                      ) : (
                        <button
                          type="button"
                          onClick={() => void quarantine(file)}
                          disabled={busyId !== null}
                          className="cv-btn cv-btn-danger shrink-0 px-2 py-0.5 text-xs disabled:opacity-50"
                        >
                          <ShieldAlert size={12} /> {busyId === file.id ? "Moving..." : "Quarantine"}
                        </button>
                      )}
                    </div>
                  ))}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
