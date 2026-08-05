import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion } from "framer-motion";
import { FolderOpen, HardDrive, Plus, RefreshCw, Scan, Square, Trash2 } from "lucide-react";
import { useAppStore } from "../../store/appStore";

type Source = { id?: number; path: string; source_type: string; name: string; enabled: boolean; last_scanned?: string; item_count: number };
type ScanResult = { total_found?: number; total_added?: number; total_updated?: number; sources_scanned?: number; sources_failed?: number; errors?: string[]; cancelled?: boolean };

export default function MediaSourcesTab() {
  const { sources, setSources, scanning, setScanning, scanProgress, addStatusMessage } = useAppStore();
  const [path, setPath] = useState("");
  const [name, setName] = useState("");
  const [sourceType, setSourceType] = useState("drive");
  const [adding, setAdding] = useState(false);
  const [stopping, setStopping] = useState(false);

  const loadSources = async () => {
    try { setSources(await invoke<Source[]>("get_sources")); }
    catch (error) { addStatusMessage(`Source list unavailable: ${error}`); }
  };

  useEffect(() => { void loadSources(); }, []);

  const stopScan = async () => {
    if (!scanning || stopping) return;
    setStopping(true);
    try {
      await invoke("cancel_scan");
      addStatusMessage("Stop requested. The scanner will finish the current file and exit safely.");
    } catch (error) {
      addStatusMessage(`Unable to stop scan: ${error}`);
    } finally {
      setStopping(false);
    }
  };

  const finish = async (result: ScanResult) => {
    const cancelled = result.cancelled ? " Scan cancelled safely." : "";
    addStatusMessage(`Scan complete: ${result.total_found || 0} found, ${result.total_added || 0} added, ${result.total_updated || 0} refreshed.${cancelled}`);
    if (result.errors?.length) addStatusMessage(`Scan warnings: ${result.errors.slice(0, 3).join("; ")}`);
    await loadSources();
    window.dispatchEvent(new CustomEvent("cinavault:library-refresh", { detail: { reason: "source-scan" } }));
  };

  const scanAll = async () => {
    if (scanning) return;
    setScanning(true);
    addStatusMessage("Scanning configured sources...");
    try { await finish(await invoke<ScanResult>("scan_sources")); }
    catch (error) { addStatusMessage(`Scan failed: ${error}`); }
    finally { setScanning(false); }
  };

  const addSource = async () => {
    const cleanPath = path.trim();
    if (!cleanPath || adding || scanning) return;
    setAdding(true);
    try {
      const sourceName = name.trim() || cleanPath;
      const sourceId = await invoke<number>("add_source", { path: cleanPath, sourceType, name: sourceName });
      setPath(""); setName("");
      await loadSources();
      setScanning(true);
      addStatusMessage(`Scanning source: ${sourceName}`);
      await finish(await invoke<ScanResult>("scan_single_source", { sourceId }));
    } catch (error) {
      addStatusMessage(`Add/scan source failed: ${error}`);
    } finally {
      setAdding(false);
      setScanning(false);
    }
  };

  const removeSource = async (id: number) => {
    try { await invoke("remove_source", { id }); await loadSources(); addStatusMessage("Source removed"); }
    catch (error) { addStatusMessage(`Source removal failed: ${error}`); }
  };

  return (
    <div className="space-y-5">
      <section className="glass-panel p-5">
        <h3 className="flex items-center gap-2 text-sm font-bold"><Plus size={16} className="text-cv-accent" /> Add and scan media</h3>
        <p className="mt-1 text-xs text-cv-subtext">Build 1.09 supports full external-drive roots such as D:\\, skips protected system folders, and reports progress while traversing.</p>
        <div className="mt-4 grid grid-cols-1 gap-3 md:grid-cols-4">
          <input value={path} onChange={(event) => setPath(event.target.value)} placeholder="D:\\ or D:\\Movies" className="cv-input md:col-span-2" />
          <input value={name} onChange={(event) => setName(event.target.value)} placeholder="External D Drive" className="cv-input" />
          <select value={sourceType} onChange={(event) => setSourceType(event.target.value)} className="cv-select"><option value="drive">Drive</option><option value="folder">Folder</option><option value="file">File</option></select>
        </div>
        <div className="mt-4 flex flex-wrap gap-2">
          <button type="button" onClick={() => void addSource()} disabled={adding || scanning || !path.trim()} className="cv-btn cv-btn-primary disabled:opacity-50"><FolderOpen size={14} /> {adding ? "Adding..." : "Add and scan"}</button>
          <button type="button" onClick={() => void scanAll()} disabled={scanning} className="cv-btn cv-btn-secondary disabled:opacity-50"><Scan size={14} /> Scan everything</button>
          {scanning && <button type="button" onClick={() => void stopScan()} disabled={stopping} className="cv-btn cv-btn-danger disabled:opacity-50"><Square size={13} /> {stopping ? "Stopping..." : "Stop scan"}</button>}
        </div>
      </section>

      {scanning && <motion.section initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} className="glass-panel p-4">
        <div className="flex items-center justify-between gap-3"><div><div className="text-sm font-semibold">Scanning external media</div><div className="text-[10px] text-cv-subtext">Protected folders are skipped; access errors do not stop the scan.</div></div><div className="text-xs tabular-nums text-cv-subtext">{scanProgress.current} / {scanProgress.total || "discovering"}</div></div>
        <div className="mt-3 h-2 overflow-hidden rounded-full bg-white/10"><motion.div className="h-full rounded-full" style={{ background: "linear-gradient(90deg, var(--cv-accent), var(--cv-neon-1))" }} animate={{ width: scanProgress.total ? `${Math.min(100, (scanProgress.current / scanProgress.total) * 100)}%` : "20%" }} /></div>
      </motion.section>}

      <section className="glass-panel overflow-hidden rounded-xl">
        <div className="flex items-center justify-between border-b border-white/5 px-5 py-3"><h3 className="text-sm font-bold">Configured sources ({sources.length})</h3><button type="button" onClick={() => void loadSources()} className="cv-btn cv-btn-secondary py-1 text-xs"><RefreshCw size={12} /> Refresh</button></div>
        {sources.length === 0 ? <div className="p-8 text-center text-sm text-cv-subtext">No sources configured.</div> : <div className="divide-y divide-white/5">{sources.map((source: Source) => <div key={source.id || source.path} className="flex items-center gap-4 px-5 py-3"><div className="grid h-10 w-10 place-items-center rounded-lg bg-cv-accent/10"><HardDrive size={18} className="text-cv-accent" /></div><div className="min-w-0 flex-1"><div className="truncate text-sm font-semibold">{source.name}</div><div className="truncate text-xs text-cv-subtext">{source.path}</div></div><div className="text-right text-xs text-cv-subtext"><div>{source.item_count} items</div><div>{source.last_scanned ? new Date(source.last_scanned).toLocaleString() : "Never scanned"}</div></div>{source.id && <button type="button" onClick={() => void removeSource(source.id!)} className="cv-btn cv-btn-danger px-2 py-1"><Trash2 size={12} /></button>}</div>)}</div>}
      </section>
    </div>
  );
}
