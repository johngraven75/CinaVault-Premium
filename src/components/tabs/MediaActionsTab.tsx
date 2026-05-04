import React, { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "../../store/appStore";
import { Bot, Search, CheckSquare, Square, Wand2, ImageIcon, Clapperboard, CalendarClock, Save } from "lucide-react";

interface MetadataProvider {
  name: string;
  key: string;
  category: string;
  requires_key: boolean;
}

interface MediaItem {
  id?: number;
  title: string;
  file_path: string;
}

export default function MediaActionsTab() {
  const { settings, setSetting, addStatusMessage } = useAppStore();
  const [providers, setProviders] = useState<MetadataProvider[]>([]);
  const [selectedProviders, setSelectedProviders] = useState<Set<string>>(new Set());
  const [metadataQuery, setMetadataQuery] = useState("");
  const [batchResults, setBatchResults] = useState<any[]>([]);
  const [batchBusy, setBatchBusy] = useState(false);
  const [correctionBusy, setCorrectionBusy] = useState(false);

  const [mediaItems, setMediaItems] = useState<MediaItem[]>([]);
  const [selectedMediaPath, setSelectedMediaPath] = useState("");
  const [chapterOutputDir, setChapterOutputDir] = useState("");
  const [chapterInterval, setChapterInterval] = useState("300");
  const [chapterBusy, setChapterBusy] = useState(false);

  const [runMode, setRunMode] = useState("when_added");
  const [scheduleTime, setScheduleTime] = useState("03:00");
  const [scheduleEveryHours, setScheduleEveryHours] = useState("24");

  const groupedProviders = useMemo(() => {
    const grouped = new Map<string, MetadataProvider[]>();
    for (const provider of providers) {
      const group = grouped.get(provider.category) ?? [];
      group.push(provider);
      grouped.set(provider.category, group);
    }
    return Array.from(grouped.entries());
  }, [providers]);

  useEffect(() => {
    loadMetadataProviders();
    loadMediaItems();
  }, []);

  useEffect(() => {
    setRunMode(settings.media_actions_run_mode || "when_added");
    setScheduleTime(settings.media_actions_schedule_time || "03:00");
    setScheduleEveryHours(settings.media_actions_interval_hours || "24");

    const raw = settings.metadata_selected_providers;
    if (raw) {
      try {
        const parsed = JSON.parse(raw) as string[];
        if (parsed.length) {
          setSelectedProviders(new Set(parsed));
        }
      } catch {
        // ignore malformed persisted settings
      }
    }
  }, [settings.media_actions_run_mode, settings.media_actions_schedule_time, settings.media_actions_interval_hours, settings.metadata_selected_providers]);

  const loadMetadataProviders = async () => {
    try {
      const loaded = await invoke<MetadataProvider[]>("get_metadata_providers");
      setProviders(loaded);
      if (!selectedProviders.size) {
        setSelectedProviders(new Set(loaded.slice(0, 4).map((p) => p.key)));
      }
    } catch {
      setProviders([]);
    }
  };

  const loadMediaItems = async () => {
    try {
      const loaded = await invoke<MediaItem[]>("get_media_items", { limit: 250 });
      setMediaItems(loaded);
      if (loaded.length) setSelectedMediaPath(loaded[0].file_path);
    } catch {
      setMediaItems([]);
    }
  };

  const persistSetting = async (key: string, value: string) => {
    setSetting(key, value);
    try {
      await invoke("set_setting", { key, value });
    } catch {
      // ignore non-tauri persistence issues
    }
  };

  const persistProviderSelection = async (next: Set<string>) => {
    const serialized = JSON.stringify(Array.from(next));
    setSelectedProviders(next);
    await persistSetting("metadata_selected_providers", serialized);
  };

  const toggleProvider = async (key: string) => {
    const next = new Set(selectedProviders);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    await persistProviderSelection(next);
  };

  const setAllProviders = async (enabled: boolean) => {
    const next = enabled ? new Set(providers.map((p) => p.key)) : new Set<string>();
    await persistProviderSelection(next);
  };

  const runBatchSearch = async () => {
    if (!metadataQuery.trim()) return;
    setBatchBusy(true);
    addStatusMessage("Running bulk metadata agent search...");
    try {
      const result = await invoke<any>("batch_search_metadata", {
        query: metadataQuery,
        providers: Array.from(selectedProviders),
      });
      setBatchResults(result.providers || []);
      addStatusMessage("Bulk metadata search complete");
    } catch (e) {
      addStatusMessage(`Bulk metadata search failed: ${e}`);
    }
    setBatchBusy(false);
  };

  const runMetadataCorrection = async () => {
    setCorrectionBusy(true);
    addStatusMessage("Running metadata correction/replacement pass...");
    try {
      const result = await invoke<any>("run_metadata_correction", {
        providers: Array.from(selectedProviders),
        limit: 60,
      });
      addStatusMessage(`Metadata correction updated ${result.updated}/${result.attempted} items`);
    } catch (e) {
      addStatusMessage(`Metadata correction failed: ${e}`);
    }
    setCorrectionBusy(false);
  };

  const runChapterGeneration = async () => {
    if (!selectedMediaPath) return;
    setChapterBusy(true);
    addStatusMessage("Generating chapter thumbnails...");
    try {
      const thumbs = await invoke<any[]>("generate_chapter_thumbs", {
        filePath: selectedMediaPath,
        outputDir: chapterOutputDir || null,
        intervalSecs: Number(chapterInterval) || 300,
        ffmpegPath: settings.ffmpeg_path || null,
      });
      addStatusMessage(`Generated ${thumbs.length} chapter images`);
    } catch (e) {
      addStatusMessage(`Chapter generation failed: ${e}`);
    }
    setChapterBusy(false);
  };

  const saveRunSettings = async () => {
    await persistSetting("media_actions_run_mode", runMode);
    await persistSetting("media_actions_schedule_time", scheduleTime);
    await persistSetting("media_actions_interval_hours", scheduleEveryHours);
    addStatusMessage("Media Actions run schedule saved");
  };

  return (
    <div className="space-y-5">
      <div className="glass-panel p-5 space-y-4">
        <h3 className="text-sm font-bold flex items-center gap-2"><Bot size={16} className="text-cv-accent" /> Bulk Metadata Agent Actions</h3>

        <div className="flex gap-2">
          <button onClick={() => setAllProviders(true)} className="cv-btn cv-btn-secondary text-xs"><CheckSquare size={12} /> Check All</button>
          <button onClick={() => setAllProviders(false)} className="cv-btn cv-btn-secondary text-xs"><Square size={12} /> Uncheck All</button>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          {groupedProviders.map(([category, list]) => (
            <div key={category} className="glass-panel-2 p-3 rounded-lg">
              <div className="text-xs font-semibold text-cv-accent mb-2">{category}</div>
              <div className="space-y-1">
                {list.map((provider) => {
                  const checked = selectedProviders.has(provider.key);
                  return (
                    <button key={provider.key} onClick={() => toggleProvider(provider.key)} className="w-full flex items-center justify-between text-left px-2 py-1 rounded hover:bg-white/5">
                      <span className="text-xs">{provider.name}</span>
                      {checked ? <CheckSquare size={12} className="text-cv-accent" /> : <Square size={12} className="text-cv-subtext" />}
                    </button>
                  );
                })}
              </div>
            </div>
          ))}
        </div>

        <div className="flex gap-2">
          <input value={metadataQuery} onChange={(e) => setMetadataQuery(e.target.value)} className="cv-input flex-1" placeholder="Run bulk metadata search (title query)" />
          <button onClick={runBatchSearch} disabled={batchBusy} className="cv-btn cv-btn-primary text-xs"><Search size={12} /> {batchBusy ? "Searching..." : "Bulk Search"}</button>
          <button onClick={runMetadataCorrection} disabled={correctionBusy} className="cv-btn cv-btn-secondary text-xs"><Wand2 size={12} /> {correctionBusy ? "Running..." : "Correct + Replace"}</button>
        </div>

        {batchResults.length > 0 && (
          <div className="glass-panel-2 p-3 rounded-lg max-h-44 overflow-y-auto space-y-1">
            {batchResults.map((result: any, idx: number) => (
              <div key={`${result.provider}-${idx}`} className="flex items-center justify-between text-xs">
                <span>{result.provider}</span>
                <span className={result.ok ? "text-green-500" : "text-cv-danger"}>{result.ok ? "OK" : result.error || "Failed"}</span>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="glass-panel p-5 space-y-4">
        <h3 className="text-sm font-bold flex items-center gap-2"><ImageIcon size={16} className="text-cv-accent" /> Poster / Thumbnail / Chapter Actions</h3>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          <div className="flex items-center justify-between glass-panel-2 p-3 rounded-lg">
            <div>
              <div className="text-sm font-semibold">Poster Image Sync</div>
              <div className="text-[10px] text-cv-subtext">Pull poster images with metadata agent updates.</div>
            </div>
            <div className={`cv-toggle ${settings.poster_sync !== "false" ? "active" : ""}`} onClick={async () => persistSetting("poster_sync", settings.poster_sync === "false" ? "true" : "false")} />
          </div>
          <div className="flex items-center justify-between glass-panel-2 p-3 rounded-lg">
            <div>
              <div className="text-sm font-semibold">Chapter Thumbnail Generation</div>
              <div className="text-[10px] text-cv-subtext">Enable chapter image generation profile-wide.</div>
            </div>
            <div className={`cv-toggle ${settings.chapter_thumbs_enabled !== "false" ? "active" : ""}`} onClick={async () => persistSetting("chapter_thumbs_enabled", settings.chapter_thumbs_enabled === "false" ? "true" : "false")} />
          </div>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          <div>
            <label className="section-label">Media File for Chapter Generation</label>
            <select value={selectedMediaPath} onChange={(e) => setSelectedMediaPath(e.target.value)} className="cv-select w-full">
              {mediaItems.map((item) => (
                <option key={item.id} value={item.file_path}>{item.title}</option>
              ))}
            </select>
          </div>
          <div>
            <label className="section-label">Output Directory (optional)</label>
            <input value={chapterOutputDir} onChange={(e) => setChapterOutputDir(e.target.value)} className="cv-input" placeholder="Leave blank to auto-generate folder" />
          </div>
          <div>
            <label className="section-label">Chapter Interval (seconds)</label>
            <input value={chapterInterval} onChange={(e) => setChapterInterval(e.target.value)} className="cv-input" />
          </div>
        </div>

        <button onClick={runChapterGeneration} disabled={chapterBusy} className="cv-btn cv-btn-primary text-xs">
          <Clapperboard size={12} /> {chapterBusy ? "Generating..." : "Generate Chapter Thumbnails"}
        </button>
      </div>

      <div className="glass-panel p-5 space-y-4">
        <h3 className="text-sm font-bold flex items-center gap-2"><CalendarClock size={16} className="text-cv-accent" /> Media Actions Run Settings</h3>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
          <div>
            <label className="section-label">When to Run</label>
            <select value={runMode} onChange={(e) => setRunMode(e.target.value)} className="cv-select w-full">
              <option value="when_added">When Media is Added</option>
              <option value="scheduled">Scheduled Task</option>
              <option value="manual">Manual Only</option>
            </select>
          </div>
          <div>
            <label className="section-label">Scheduled Time (HH:MM)</label>
            <input value={scheduleTime} onChange={(e) => setScheduleTime(e.target.value)} className="cv-input" placeholder="03:00" />
          </div>
          <div>
            <label className="section-label">Schedule Every (hours)</label>
            <input value={scheduleEveryHours} onChange={(e) => setScheduleEveryHours(e.target.value)} className="cv-input" placeholder="24" />
          </div>
        </div>
        <button onClick={saveRunSettings} className="cv-btn cv-btn-primary text-xs"><Save size={12} /> Save Run Settings</button>
      </div>
    </div>
  );
}
