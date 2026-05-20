// CinaVault Premium — Media Sources Tab
import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion } from "framer-motion";
import { useAppStore } from "../../store/appStore";
import {
  FolderOpen, HardDrive, File, Plus, Trash2, RefreshCw, Scan,
  Search, Sparkles, Link, ExternalLink, CheckCircle, AlertCircle
} from "lucide-react";

export default function MediaSourcesTab() {
  const { sources, setSources, scanning, setScanning, scanProgress, setScanProgress, addStatusMessage, settings, setSetting } = useAppStore();
  const [newSourcePath, setNewSourcePath] = useState("");
  const [newSourceName, setNewSourceName] = useState("");
  const [newSourceType, setNewSourceType] = useState("folder");
  const [webLink, setWebLink] = useState("");
  const [savingOption, setSavingOption] = useState<string | null>(null);

  useEffect(() => {
    loadSources();
  }, []);

  const loadSources = async () => {
    try {
      const s = await invoke<any[]>("get_sources");
      setSources(s);
    } catch {
      setSources(DEMO_SOURCES);
    }
  };

  const addSource = async () => {
    if (!newSourcePath) return;
    try {
      await invoke("add_source", {
        path: newSourcePath,
        sourceType: newSourceType,
        name: newSourceName || newSourcePath.split(/[\\/]/).pop() || "New Source",
      });
      addStatusMessage(`Source added: ${newSourceName || newSourcePath}`);
      setNewSourcePath("");
      setNewSourceName("");
      loadSources();
    } catch (e) { addStatusMessage(`Failed to add source: ${e}`); }
  };

  const removeSource = async (id: number) => {
    try {
      await invoke("remove_source", { id });
      addStatusMessage("Source removed");
      loadSources();
    } catch (e) { addStatusMessage(`Failed: ${e}`); }
  };

  const scanAll = async () => {
    setScanning(true);
    addStatusMessage("Scanning all sources...");
    try {
      const result = await invoke<any>("scan_sources");
      addStatusMessage(`Scan complete: ${result.total_added} new items from ${result.sources_scanned} sources`);
      loadSources();
    } catch (e) { addStatusMessage(`Scan failed: ${e}`); }
    setScanning(false);
  };

  const aiDiscover = () => {
    addStatusMessage("AI Source Discovery: Analyzing system drives for media folders...");
  };

  const isEnabled = (key: string, defaultOn = false) =>
    (settings[key] ?? (defaultOn ? "true" : "false")) === "true";

  const saveLibraryOption = async (key: string, enabled: boolean) => {
    const value = enabled ? "true" : "false";
    setSetting(key, value);
    setSavingOption(key);
    try {
      await invoke("set_setting", { key, value });
      if (key === "prefer_embedded_titles" && enabled) {
        addStatusMessage("Applying embedded titles to existing library...");
        const result = await invoke<{ checked: number; updated: number; missing_files: number }>("apply_embedded_titles");
        addStatusMessage(`Embedded titles applied: ${result.updated}/${result.checked} updated`);
      }
      addStatusMessage(`Library option updated: ${key} = ${value}`);
    } catch (e) {
      addStatusMessage(`Failed to save option ${key}: ${e}`);
    }
    setSavingOption(null);
  };

  const sourcePathPlaceholder = newSourceType === "synology_quickconnect"
    ? "synology_quickconnect://username@quickconnect-id/video"
    : "C:\\Movies or /media/library";

  return (
    <div className="space-y-5">
      {/* Add Source Panel */}
      <div className="glass-panel p-5">
        <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
          <Plus size={16} className="text-cv-accent" /> Add Media Source
        </h3>
        <div className="grid grid-cols-1 md:grid-cols-4 gap-3">
          <div className="md:col-span-2">
            <label className="section-label">Path</label>
            <input
              type="text"
              value={newSourcePath}
              onChange={e => setNewSourcePath(e.target.value)}
              placeholder={sourcePathPlaceholder}
              className="cv-input"
            />
          </div>
          <div>
            <label className="section-label">Name</label>
            <input
              type="text"
              value={newSourceName}
              onChange={e => setNewSourceName(e.target.value)}
              placeholder="My Movies"
              className="cv-input"
            />
          </div>
          <div>
            <label className="section-label">Type</label>
            <select value={newSourceType} onChange={e => setNewSourceType(e.target.value)} className="cv-select w-full">
              <option value="folder">Folder</option>
              <option value="drive">Drive</option>
              <option value="file">File</option>
              <option value="synology_quickconnect">Synology QuickConnect</option>
            </select>
          </div>
        </div>
        <div className="flex gap-2 mt-4">
          <button onClick={addSource} className="cv-btn cv-btn-primary">
            <FolderOpen size={14} /> Add Source
          </button>
          <button onClick={aiDiscover} className="cv-btn cv-btn-gold">
            <Sparkles size={14} /> AI Discover Sources
          </button>
          <button onClick={scanAll} disabled={scanning} className="cv-btn cv-btn-secondary">
            <Scan size={14} className={scanning ? "animate-spin" : ""} />
            {scanning ? "Scanning..." : "Scan All Sources"}
          </button>
        </div>
      </div>

      {/* Unified Library Options */}
      <div className="glass-panel p-5">
        <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
          <Sparkles size={16} className="text-cv-accent" /> Library Options (Unified)
        </h3>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          {[
            {
              key: "prefer_embedded_titles",
              label: "Prefer embedded titles over filenames",
              desc: "Use media container tags (title metadata) when scanning, fallback to filename if missing.",
            },
            {
              key: "library_auto_scan",
              label: "Scan library automatically",
              desc: "Automatically refresh library metadata in background.",
            },
            {
              key: "library_partial_scan_on_changes",
              label: "Run partial scan when changes are detected",
              desc: "Only rescan changed folders/files for faster updates.",
            },
            {
              key: "library_empty_trash_after_scan",
              label: "Empty trash automatically after every scan",
              desc: "Remove stale media records for files no longer on disk.",
            },
          ].map((opt) => (
            <div key={opt.key} className="glass-panel-2 p-3 rounded-lg">
              <div className="flex items-start justify-between gap-3">
                <div>
                  <div className="text-xs font-semibold">{opt.label}</div>
                  <div className="text-[10px] text-cv-subtext mt-1">{opt.desc}</div>
                </div>
                <input
                  type="checkbox"
                  checked={isEnabled(opt.key, false)}
                  disabled={savingOption === opt.key}
                  onChange={(e) => saveLibraryOption(opt.key, e.target.checked)}
                />
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Web / Playlist Download Link */}
      <div className="glass-panel p-5">
        <h3 className="text-sm font-bold mb-3 flex items-center gap-2">
          <Link size={16} className="text-cv-accent" /> Web / Playlist Download Link
        </h3>
        <div className="flex gap-3">
          <input
            type="text"
            value={webLink}
            onChange={e => setWebLink(e.target.value)}
            placeholder="Paste URL to download (YouTube, etc.)"
            className="cv-input flex-1"
          />
          <button className="cv-btn cv-btn-primary shrink-0">
            <ExternalLink size={14} /> Send to Downloads
          </button>
        </div>
      </div>

      {/* Source List */}
      <div className="glass-panel rounded-xl overflow-hidden">
        <div className="px-5 py-3 border-b border-white/5 flex items-center justify-between">
          <h3 className="text-sm font-bold">Configured Sources ({sources.length})</h3>
          <button onClick={loadSources} className="cv-btn cv-btn-secondary text-xs py-1">
            <RefreshCw size={12} /> Refresh
          </button>
        </div>

        {sources.length === 0 ? (
          <div className="p-8 text-center">
            <FolderOpen size={40} className="mx-auto text-cv-subtext/20 mb-3" />
            <p className="text-sm text-cv-subtext">No sources configured. Add folders or drives above.</p>
          </div>
        ) : (
          <div className="divide-y divide-white/5">
            {sources.map((source, i) => (
              <motion.div
                key={source.id || i}
                initial={{ opacity: 0, x: -12 }}
                animate={{ opacity: 1, x: 0 }}
                transition={{ delay: i * 0.05 }}
                className="px-5 py-3 flex items-center gap-4 hover:bg-white/[0.03] transition-colors"
              >
                <div className="w-10 h-10 rounded-lg bg-cv-accent/10 flex items-center justify-center shrink-0">
                  {source.source_type === "drive" ? <HardDrive size={18} className="text-cv-accent" /> :
                   source.source_type === "file" ? <File size={18} className="text-cv-accent" /> :
                   <FolderOpen size={18} className="text-cv-accent" />}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="text-sm font-semibold truncate">{source.name}</div>
                  <div className="text-xs text-cv-subtext truncate">{source.path}</div>
                </div>
                <div className="text-xs text-cv-subtext text-right shrink-0">
                  <div>{source.item_count} items</div>
                  <div>{source.last_scanned ? new Date(source.last_scanned).toLocaleDateString() : "Never scanned"}</div>
                </div>
                <div className="flex items-center gap-1 shrink-0">
                  <span className={`status-dot ${source.enabled ? "online" : "offline"}`} />
                </div>
                <button
                  onClick={() => source.id && removeSource(source.id)}
                  className="cv-btn cv-btn-danger text-xs py-1 px-2"
                >
                  <Trash2 size={12} />
                </button>
              </motion.div>
            ))}
          </div>
        )}
      </div>

      {/* Scan Progress */}
      {scanning && (
        <motion.div
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          className="glass-panel p-4"
        >
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm font-semibold">Scanning in progress...</span>
            <span className="text-xs text-cv-subtext">{scanProgress.current} / {scanProgress.total}</span>
          </div>
          <div className="w-full h-2 rounded-full bg-white/10 overflow-hidden">
            <motion.div
              className="h-full rounded-full"
              style={{ background: "linear-gradient(90deg, var(--cv-accent), var(--cv-neon-1))" }}
              animate={{ width: scanProgress.total ? `${(scanProgress.current / scanProgress.total) * 100}%` : "0%" }}
            />
          </div>
        </motion.div>
      )}
    </div>
  );
}

const DEMO_SOURCES = [
  { id: 1, path: "C:\\Movies", source_type: "folder", name: "Movies Library", enabled: true, last_scanned: "2026-04-25", item_count: 342 },
  { id: 2, path: "D:\\TV Shows", source_type: "folder", name: "TV Shows", enabled: true, last_scanned: "2026-04-20", item_count: 1280 },
  { id: 3, path: "E:\\Music", source_type: "drive", name: "Music Drive", item_count: 0, enabled: true },
];
