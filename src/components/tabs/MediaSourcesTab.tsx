// CinaVault Premium - Media Sources Tab
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion } from "framer-motion";
import {
  useAppStore,
  type LibraryEnrichmentResult,
} from "../../store/appStore";
import {
  FolderOpen,
  HardDrive,
  File,
  Plus,
  Trash2,
  RefreshCw,
  Scan,
  Sparkles,
  Link,
  ExternalLink,
} from "lucide-react";

type ScanResult = {
  total_found?: number | string;
  total_added?: number | string;
  sources_scanned?: number | string;
};

type SourceLike = {
  id?: number;
  path: string;
  source_type: string;
  name: string;
  enabled: boolean;
  last_scanned?: string;
  item_count: number;
};

const DEFAULT_METADATA_AFTER_SCAN = true;

const DEMO_SOURCES: SourceLike[] = [
  {
    id: 1,
    path: "C:\\Movies",
    source_type: "folder",
    name: "Movies Library",
    enabled: true,
    last_scanned: "2026-04-25",
    item_count: 342,
  },
  {
    id: 2,
    path: "D:\\TV Shows",
    source_type: "folder",
    name: "TV Shows",
    enabled: true,
    last_scanned: "2026-04-20",
    item_count: 1280,
  },
  {
    id: 3,
    path: "E:\\Music",
    source_type: "drive",
    name: "Music Drive",
    item_count: 0,
    enabled: true,
  },
];

function safeNumber(value: unknown): number {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && value.trim()) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : 0;
  }
  return 0;
}

function formatMetadataSummary(result: LibraryEnrichmentResult): string {
  const enriched =
    result.metadata_items_enriched || result.metadata_updated || 0;
  const fields = result.metadata_fields_updated || 0;
  const skipped =
    (result.low_confidence_metadata_only || 0) +
    (result.skipped_missing_files || 0);
  const warnings = result.provider_errors?.length || 0;
  return [
    `Metadata pull complete: ${enriched} items enriched`,
    `${fields} fields updated`,
    `${skipped} skipped`,
    warnings ? `${warnings} provider warnings` : "no provider warnings",
  ].join(", ");
}

function sourceIcon(sourceType: string) {
  if (sourceType === "drive")
    return <HardDrive size={18} className="text-cv-accent" />;
  if (sourceType === "file")
    return <File size={18} className="text-cv-accent" />;
  return <FolderOpen size={18} className="text-cv-accent" />;
}

export default function MediaSourcesTab() {
  const {
    sources,
    setSources,
    scanning,
    setScanning,
    scanProgress,
    addStatusMessage,
    settings,
    setSetting,
    scheduledTasks,
  } = useAppStore();

  const [newSourcePath, setNewSourcePath] = useState("");
  const [newSourceName, setNewSourceName] = useState("");
  const [newSourceType, setNewSourceType] = useState("folder");
  const [webLink, setWebLink] = useState("");
  const [savingOption, setSavingOption] = useState<string | null>(null);
  const [discovering, setDiscovering] = useState(false);

  useEffect(() => {
    void loadSources();
  }, []);

  const loadSources = async () => {
    try {
      const loaded = await invoke<SourceLike[]>("get_sources");
      setSources(loaded);
    } catch {
      setSources(DEMO_SOURCES);
    }
  };

  const addSource = async () => {
    const path = newSourcePath.trim();
    if (!path) return;
    try {
      await invoke("add_source", {
        path,
        sourceType: newSourceType,
        name: newSourceName.trim() || path.split(/[\\/]/).pop() || "New Source",
      });
      addStatusMessage(`Source added: ${newSourceName.trim() || path}`);
      setNewSourcePath("");
      setNewSourceName("");
      await loadSources();
    } catch (error) {
      addStatusMessage(`Failed to add source: ${error}`);
    }
  };

  const removeSource = async (id: number) => {
    try {
      await invoke("remove_source", { id });
      addStatusMessage("Source removed");
      await loadSources();
    } catch (error) {
      addStatusMessage(`Failed to remove source: ${error}`);
    }
  };

  const isEnabled = (key: string, defaultOn = false) => {
    return (settings[key] ?? (defaultOn ? "true" : "false")) === "true";
  };

  const shouldPullMetadataAfterScan = (scanResult: ScanResult) => {
    const found = safeNumber(scanResult.total_found);
    if (found <= 0) return false;
    return (
      isEnabled("library_auto_scan", DEFAULT_METADATA_AFTER_SCAN) ||
      scheduledTasks.metadata_check === "on_scan"
    );
  };

  const pullMetadataAfterScan = async (scanResult: ScanResult) => {
    if (!shouldPullMetadataAfterScan(scanResult)) {
      addStatusMessage(
        "Metadata pull skipped: automatic metadata after scan is disabled",
      );
      return;
    }
    addStatusMessage("Pulling metadata for scanned media...");
    const enrichment = await invoke<LibraryEnrichmentResult>(
      "run_library_enrichment",
      { renameFiles: false },
    );
    addStatusMessage(formatMetadataSummary(enrichment));
  };

  const scanAll = async () => {
    if (scanning) return;
    setScanning(true);
    addStatusMessage("Scanning all sources...");
    try {
      const result = await invoke<ScanResult>("scan_sources");
      const added = safeNumber(result.total_added);
      const scanned = safeNumber(result.sources_scanned);
      addStatusMessage(
        `Scan complete: ${added} new items from ${scanned} sources`,
      );
      await loadSources();
      try {
        await pullMetadataAfterScan(result);
        await loadSources();
      } catch (metadataError) {
        addStatusMessage(`Metadata pull after scan failed: ${metadataError}`);
      }
    } catch (scanError) {
      addStatusMessage(`Scan failed: ${scanError}`);
    } finally {
      setScanning(false);
    }
  };

  const aiDiscover = async () => {
    if (discovering) return;
    setDiscovering(true);
    addStatusMessage(
      "AI Source Discovery: analyzing system drives for media folders...",
    );
    try {
      const result = await invoke<{
        status: string;
        roots_checked: number;
        discovered: number;
        added: number;
        existing: number;
        paths: string[];
        message: string;
      }>("discover_media_sources");
      addStatusMessage(
        `AI Source Discovery complete: ${result.discovered} folders found, ${result.added} added, ${result.existing} already configured`,
      );
      await loadSources();
    } catch (error) {
      addStatusMessage(`AI Source Discovery failed: ${error}`);
    } finally {
      setDiscovering(false);
    }
  };

  const saveLibraryOption = async (key: string, enabled: boolean) => {
    const value = enabled ? "true" : "false";
    setSetting(key, value);
    setSavingOption(key);
    try {
      await invoke("set_setting", { key, value });
      if (key === "prefer_embedded_titles" && enabled) {
        addStatusMessage("Applying embedded titles to existing library...");
        const result = await invoke<{
          checked: number;
          updated: number;
          missing_files: number;
        }>("apply_embedded_titles");
        addStatusMessage(
          `Embedded titles applied: ${result.updated}/${result.checked} updated`,
        );
      }
      addStatusMessage(`Library option updated: ${key} = ${value}`);
    } catch (error) {
      addStatusMessage(`Failed to save option ${key}: ${error}`);
    } finally {
      setSavingOption(null);
    }
  };

  return (
    <div className="space-y-5">
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
              onChange={(event) => setNewSourcePath(event.target.value)}
              placeholder="C:\\Movies or /media/library"
              className="cv-input"
            />
          </div>
          <div>
            <label className="section-label">Name</label>
            <input
              type="text"
              value={newSourceName}
              onChange={(event) => setNewSourceName(event.target.value)}
              placeholder="My Movies"
              className="cv-input"
            />
          </div>
          <div>
            <label className="section-label">Type</label>
            <select
              value={newSourceType}
              onChange={(event) => setNewSourceType(event.target.value)}
              className="cv-select w-full"
            >
              <option value="folder">Folder</option>
              <option value="drive">Drive</option>
              <option value="file">File</option>
            </select>
          </div>
        </div>
        <div className="flex gap-2 mt-4">
          <button onClick={addSource} className="cv-btn cv-btn-primary">
            <FolderOpen size={14} /> Add Source
          </button>
          <button
            onClick={aiDiscover}
            disabled={discovering}
            className="cv-btn cv-btn-gold disabled:opacity-50"
          >
            <Sparkles size={14} className={discovering ? "animate-spin" : ""} />
            {discovering ? "Discovering Sources..." : "AI Discover Sources"}
          </button>
          <button
            onClick={scanAll}
            disabled={scanning}
            className="cv-btn cv-btn-secondary"
          >
            <Scan size={14} className={scanning ? "animate-spin" : ""} />
            {scanning ? "Scanning + Pulling Metadata..." : "Scan All Sources"}
          </button>
        </div>
      </div>

      <div className="glass-panel p-5">
        <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
          <Sparkles size={16} className="text-cv-accent" /> Library Options
          (Unified)
        </h3>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          {[
            [
              "prefer_embedded_titles",
              "Prefer embedded titles over filenames",
              "Use media container tags when scanning, then fall back to filenames.",
              false,
            ],
            [
              "library_auto_scan",
              "Pull metadata automatically after scans",
              "After indexing source files, run native enrichment for titles, posters, years, ratings, genres, and IDs.",
              true,
            ],
            [
              "library_partial_scan_on_changes",
              "Run partial scan when changes are detected",
              "Only rescan changed folders/files for faster updates.",
              false,
            ],
            [
              "library_empty_trash_after_scan",
              "Empty trash automatically after every scan",
              "Remove stale media records for files no longer on disk.",
              false,
            ],
          ].map(([key, label, desc, defaultOn]) => (
            <div key={String(key)} className="glass-panel-2 p-3 rounded-lg">
              <div className="flex items-start justify-between gap-3">
                <div>
                  <div className="text-xs font-semibold">{String(label)}</div>
                  <div className="text-[10px] text-cv-subtext mt-1">
                    {String(desc)}
                  </div>
                </div>
                <input
                  type="checkbox"
                  checked={isEnabled(String(key), Boolean(defaultOn))}
                  disabled={savingOption === key}
                  onChange={(event) =>
                    saveLibraryOption(String(key), event.target.checked)
                  }
                />
              </div>
            </div>
          ))}
        </div>
      </div>

      <div className="glass-panel p-5">
        <h3 className="text-sm font-bold mb-3 flex items-center gap-2">
          <Link size={16} className="text-cv-accent" /> Web / Playlist Download
          Link
        </h3>
        <div className="flex gap-3">
          <input
            type="text"
            value={webLink}
            onChange={(event) => setWebLink(event.target.value)}
            placeholder="Paste URL to download (YouTube, etc.)"
            className="cv-input flex-1"
          />
          <button className="cv-btn cv-btn-primary shrink-0">
            <ExternalLink size={14} /> Send to Downloads
          </button>
        </div>
      </div>

      <div className="glass-panel rounded-xl overflow-hidden">
        <div className="px-5 py-3 border-b border-white/5 flex items-center justify-between">
          <h3 className="text-sm font-bold">
            Configured Sources ({sources.length})
          </h3>
          <button
            onClick={loadSources}
            className="cv-btn cv-btn-secondary text-xs py-1"
          >
            <RefreshCw size={12} /> Refresh
          </button>
        </div>

        {sources.length === 0 ? (
          <div className="p-8 text-center">
            <FolderOpen size={40} className="mx-auto text-cv-subtext/20 mb-3" />
            <p className="text-sm text-cv-subtext">
              No sources configured. Add folders or drives above.
            </p>
          </div>
        ) : (
          <div className="divide-y divide-white/5">
            {sources.map((source, index) => (
              <motion.div
                key={source.id || index}
                initial={{ opacity: 0, x: -12 }}
                animate={{ opacity: 1, x: 0 }}
                transition={{ delay: index * 0.05 }}
                className="px-5 py-3 flex items-center gap-4 hover:bg-white/[0.03] transition-colors"
              >
                <div className="w-10 h-10 rounded-lg bg-cv-accent/10 flex items-center justify-center shrink-0">
                  {sourceIcon(source.source_type)}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="text-sm font-semibold truncate">
                    {source.name}
                  </div>
                  <div className="text-xs text-cv-subtext truncate">
                    {source.path}
                  </div>
                </div>
                <div className="text-xs text-cv-subtext text-right shrink-0">
                  <div>{source.item_count} items</div>
                  <div>
                    {source.last_scanned
                      ? new Date(source.last_scanned).toLocaleDateString()
                      : "Never scanned"}
                  </div>
                </div>
                <div className="flex items-center gap-1 shrink-0">
                  <span
                    className={`status-dot ${source.enabled ? "online" : "offline"}`}
                  />
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

      {scanning && (
        <motion.div
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          className="glass-panel p-4"
        >
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm font-semibold">
              Scanning and pulling metadata...
            </span>
            <span className="text-xs text-cv-subtext">
              {scanProgress.current} / {scanProgress.total}
            </span>
          </div>
          <div className="w-full h-2 rounded-full bg-white/10 overflow-hidden">
            <motion.div
              className="h-full rounded-full"
              style={{
                background:
                  "linear-gradient(90deg, var(--cv-accent), var(--cv-neon-1))",
              }}
              animate={{
                width: scanProgress.total
                  ? `${(scanProgress.current / scanProgress.total) * 100}%`
                  : "0%",
              }}
            />
          </div>
        </motion.div>
      )}
    </div>
  );
}
