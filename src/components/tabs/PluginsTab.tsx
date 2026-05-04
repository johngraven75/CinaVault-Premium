// CinaVault Premium - Plugins & Metadata Tab
import React, { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion } from "framer-motion";
import { useAppStore } from "../../store/appStore";
import {
  Puzzle, Plus, RefreshCw, Download, CheckCircle, XCircle,
  Key, TestTube, Search, Database, ChevronDown, ChevronRight,
  CheckSquare, Square, Wand2, Save, Trash2
} from "lucide-react";

interface MetadataProvider {
  name: string;
  key: string;
  base_url: string;
  requires_key: boolean;
  category: string;
}

interface MediaItem {
  id?: number;
  title: string;
  year?: number;
  overview?: string;
  genre?: string;
  rating?: number;
  tmdb_id?: string;
  imdb_id?: string;
}

export default function PluginsTab() {
  const { addStatusMessage, settings, setSetting } = useAppStore();
  const [repos, setRepos] = useState<any[]>([]);
  const [plugins, setPlugins] = useState<any[]>([]);
  const [newRepo, setNewRepo] = useState({ name: "", url: "" });
  const [showAddRepo, setShowAddRepo] = useState(false);
  const [apiKeys, setApiKeys] = useState<Record<string, string>>({});
  const [editingKey, setEditingKey] = useState<string | null>(null);
  const [keyValue, setKeyValue] = useState("");
  const [expandedCats, setExpandedCats] = useState<Set<string>>(new Set(["Movies & TV"]));
  const [activeSection, setActiveSection] = useState<"plugins" | "metadata">("metadata");

  const [providers, setProviders] = useState<MetadataProvider[]>([]);
  const [selectedProviders, setSelectedProviders] = useState<Set<string>>(new Set());
  const [query, setQuery] = useState("");
  const [batchBusy, setBatchBusy] = useState(false);
  const [batchResults, setBatchResults] = useState<any[]>([]);
  const [correctionBusy, setCorrectionBusy] = useState(false);
  const [unverifiedMedia, setUnverifiedMedia] = useState<MediaItem[]>([]);
  const [replacement, setReplacement] = useState({
    media_id: 0,
    title: "",
    year: "",
    rating: "",
    overview: "",
    genre: "",
    tmdb_id: "",
    imdb_id: "",
  });

  useEffect(() => {
    loadRepos();
    loadApiKeys();
    loadMetadataProviders();
    loadUnverifiedMedia();
  }, []);

  useEffect(() => {
    if (!providers.length) return;
    const raw = settings.metadata_selected_providers;
    if (raw) {
      try {
        const parsed = JSON.parse(raw) as string[];
        if (parsed.length) {
          setSelectedProviders(new Set(parsed));
          return;
        }
      } catch {
        // ignore malformed persisted value
      }
    }
    setSelectedProviders(new Set(providers.map((p) => p.key)));
  }, [providers, settings.metadata_selected_providers]);

  const providersByCategory = useMemo(() => {
    const grouped = new Map<string, MetadataProvider[]>();
    providers.forEach((p) => {
      const list = grouped.get(p.category) ?? [];
      list.push(p);
      grouped.set(p.category, list);
    });
    return Array.from(grouped.entries()).map(([category, list]) => ({ category, providers: list }));
  }, [providers]);

  const loadRepos = async () => {
    try {
      setRepos(await invoke<any[]>("get_plugin_repos"));
    } catch {
      setRepos(DEMO_REPOS);
    }
  };

  const loadPlugins = async (repoId?: number) => {
    try {
      setPlugins(await invoke<any[]>("get_plugin_catalog", { repoId }));
    } catch {
      setPlugins(DEMO_PLUGINS);
    }
  };

  const loadMetadataProviders = async () => {
    try {
      setProviders(await invoke<MetadataProvider[]>("get_metadata_providers"));
    } catch {
      setProviders([]);
    }
  };

  const loadApiKeys = async () => {
    try {
      setApiKeys(await invoke<Record<string, string>>("get_api_keys"));
    } catch {
      setApiKeys({});
    }
  };

  const loadUnverifiedMedia = async () => {
    try {
      const items = await invoke<MediaItem[]>("get_unverified_media");
      setUnverifiedMedia(items);
      if (items.length && !replacement.media_id) {
        const first = items[0];
        setReplacement((prev) => ({
          ...prev,
          media_id: first.id || 0,
          title: first.title || "",
          year: first.year ? String(first.year) : "",
          overview: first.overview || "",
          genre: first.genre || "",
          rating: first.rating ? String(first.rating) : "",
          tmdb_id: first.tmdb_id || "",
          imdb_id: first.imdb_id || "",
        }));
      }
    } catch {
      setUnverifiedMedia([]);
    }
  };

  const persistSelectedProviders = async (next: Set<string>) => {
    const serialized = JSON.stringify(Array.from(next));
    setSetting("metadata_selected_providers", serialized);
    try {
      await invoke("set_setting", { key: "metadata_selected_providers", value: serialized });
    } catch {
      // ignore persistence failure in dev mode
    }
  };

  const toggleProvider = async (key: string) => {
    const next = new Set(selectedProviders);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    setSelectedProviders(next);
    await persistSelectedProviders(next);
  };

  const selectAllProviders = async (enabled: boolean) => {
    const next = enabled ? new Set(providers.map((p) => p.key)) : new Set<string>();
    setSelectedProviders(next);
    await persistSelectedProviders(next);
  };

  const addRepo = async () => {
    if (!newRepo.name || !newRepo.url) return;
    try {
      await invoke("add_plugin_repo", newRepo);
      addStatusMessage(`Repo added: ${newRepo.name}`);
      setNewRepo({ name: "", url: "" });
      setShowAddRepo(false);
      loadRepos();
    } catch (e) {
      addStatusMessage(`Failed: ${e}`);
    }
  };

  const syncRepo = async (id: number) => {
    addStatusMessage("Syncing plugin catalog...");
    try {
      const result = await invoke<any>("sync_plugin_catalog", { repoId: id });
      addStatusMessage(`Synced ${result.plugins_synced} plugins`);
      loadPlugins(id);
    } catch (e) {
      addStatusMessage(`Sync failed: ${e}`);
    }
  };

  const removeRepo = async (id: number) => {
    try {
      await invoke("remove_plugin_repo", { repoId: id });
      addStatusMessage("Repository removed");
      if (repos.length === 1) setPlugins([]);
      loadRepos();
    } catch (e) {
      addStatusMessage(`Failed to remove repository: ${e}`);
    }
  };

  const installPlugin = async (id: number) => {
    try {
      await invoke("install_plugin", { id });
      addStatusMessage("Plugin installed");
      loadPlugins();
    } catch (e) {
      addStatusMessage(`Failed: ${e}`);
    }
  };

  const saveApiKey = async (provider: string) => {
    try {
      await invoke("set_api_key", { provider, apiKey: keyValue });
      addStatusMessage(`API key saved for ${provider}`);
      setEditingKey(null);
      setKeyValue("");
      loadApiKeys();
    } catch (e) {
      addStatusMessage(`Failed: ${e}`);
    }
  };

  const testApiKey = async (provider: string) => {
    addStatusMessage(`Testing API key for ${provider}...`);
    try {
      const result = await invoke<any>("test_api_key", { provider, apiKey: keyValue || "test" });
      addStatusMessage(`${provider}: ${result.valid ? "Valid" : "Invalid"}`);
    } catch (e) {
      addStatusMessage(`Test failed: ${e}`);
    }
  };

  const runBatchSearch = async () => {
    if (!query.trim()) return;
    setBatchBusy(true);
    addStatusMessage("Running metadata batch search...");
    try {
      const result = await invoke<any>("batch_search_metadata", {
        query,
        providers: Array.from(selectedProviders),
        mediaType: "movie",
      });
      setBatchResults(result.providers || []);
      addStatusMessage("Batch metadata search complete");
    } catch (e) {
      addStatusMessage(`Batch metadata search failed: ${e}`);
    }
    setBatchBusy(false);
  };

  const runAutoCorrection = async () => {
    setCorrectionBusy(true);
    addStatusMessage("Running metadata correction/replacement...");
    try {
      const result = await invoke<any>("run_metadata_correction", {
        providers: Array.from(selectedProviders),
        limit: 30,
      });
      addStatusMessage(`Correction complete: ${result.updated}/${result.attempted} updated`);
      loadUnverifiedMedia();
    } catch (e) {
      addStatusMessage(`Metadata correction failed: ${e}`);
    }
    setCorrectionBusy(false);
  };

  const applyManualReplacement = async () => {
    if (!replacement.media_id) return;
    try {
      await invoke("replace_media_metadata", {
        replacement: {
          media_id: replacement.media_id,
          title: replacement.title || null,
          year: replacement.year ? Number(replacement.year) : null,
          rating: replacement.rating ? Number(replacement.rating) : null,
          overview: replacement.overview || null,
          genre: replacement.genre || null,
          tmdb_id: replacement.tmdb_id || null,
          imdb_id: replacement.imdb_id || null,
          verified: true,
        },
      });
      addStatusMessage("Metadata replacement saved");
      loadUnverifiedMedia();
    } catch (e) {
      addStatusMessage(`Metadata replacement failed: ${e}`);
    }
  };

  const toggleCat = (name: string) => {
    setExpandedCats((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  };

  return (
    <div className="space-y-5">
      <div className="flex gap-2">
        <button onClick={() => setActiveSection("metadata")} className={`cv-btn ${activeSection === "metadata" ? "cv-btn-primary" : "cv-btn-secondary"}`}>
          <Database size={14} /> Metadata Providers
        </button>
        <button onClick={() => setActiveSection("plugins")} className={`cv-btn ${activeSection === "plugins" ? "cv-btn-primary" : "cv-btn-secondary"}`}>
          <Puzzle size={14} /> Plugin System
        </button>
      </div>

      {activeSection === "metadata" ? (
        <>
          <div className="glass-panel p-5">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-sm font-bold flex items-center gap-2">
                <Database size={16} className="text-cv-accent" /> Metadata Providers ({providers.length})
              </h3>
              <div className="flex gap-2">
                <button onClick={() => selectAllProviders(true)} className="cv-btn cv-btn-secondary text-xs">
                  <CheckSquare size={12} /> Check All
                </button>
                <button onClick={() => selectAllProviders(false)} className="cv-btn cv-btn-secondary text-xs">
                  <Square size={12} /> Uncheck All
                </button>
              </div>
            </div>

            <div className="space-y-2">
              {providersByCategory.map((cat) => (
                <div key={cat.category} className="glass-panel-2 rounded-lg overflow-hidden">
                  <button onClick={() => toggleCat(cat.category)} className="w-full px-4 py-3 flex items-center justify-between hover:bg-white/[0.03]">
                    <div className="flex items-center gap-2">
                      <span className="text-xs font-bold uppercase tracking-wider text-cv-accent">{cat.category}</span>
                    </div>
                    <div className="flex items-center gap-2">
                      <span className="text-[10px] text-cv-subtext">{cat.providers.length} providers</span>
                      {expandedCats.has(cat.category) ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                    </div>
                  </button>
                  {expandedCats.has(cat.category) && (
                    <div className="px-4 pb-3 space-y-1">
                      {cat.providers.map((provider) => {
                        const checked = selectedProviders.has(provider.key);
                        const hasKey = !!apiKeys[provider.key];
                        return (
                          <div key={provider.key} className="flex items-center justify-between py-2 px-3 rounded hover:bg-white/[0.02] gap-2">
                            <div className="flex items-center gap-2 min-w-0">
                              <button onClick={() => toggleProvider(provider.key)} className="text-cv-subtext hover:text-cv-text shrink-0">
                                {checked ? <CheckSquare size={14} className="text-cv-accent" /> : <Square size={14} />}
                              </button>
                              {hasKey ? <CheckCircle size={12} className="text-green-500 shrink-0" /> : <XCircle size={12} className="text-cv-subtext/30 shrink-0" />}
                              <span className="text-sm truncate">{provider.name}</span>
                              {hasKey && <span className="text-[9px] px-1.5 py-0.5 rounded bg-green-500/10 text-green-500">Configured</span>}
                            </div>
                            <div className="flex gap-1 shrink-0">
                              {editingKey === provider.key ? (
                                <>
                                  <input value={keyValue} onChange={(e) => setKeyValue(e.target.value)} className="cv-input text-xs py-1 w-40" placeholder="Enter API key" />
                                  <button onClick={() => saveApiKey(provider.key)} className="cv-btn cv-btn-primary text-[10px] py-1 px-2"><Key size={10} /> Save</button>
                                  <button onClick={() => testApiKey(provider.key)} className="cv-btn cv-btn-secondary text-[10px] py-1 px-2"><TestTube size={10} /></button>
                                  <button onClick={() => setEditingKey(null)} className="cv-btn cv-btn-secondary text-[10px] py-1 px-2">Cancel</button>
                                </>
                              ) : (
                                <button onClick={() => { setEditingKey(provider.key); setKeyValue(""); }} className="cv-btn cv-btn-secondary text-[10px] py-1 px-2">
                                  <Key size={10} /> {hasKey ? "Update" : "Add Key"}
                                </button>
                              )}
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  )}
                </div>
              ))}
            </div>
          </div>

          <div className="glass-panel p-5 space-y-4">
            <h3 className="text-sm font-bold flex items-center gap-2">
              <Search size={16} className="text-cv-accent" /> Metadata Search / Correction / Replacement
            </h3>
            <div className="flex gap-2">
              <input
                className="cv-input flex-1"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Search title for batch metadata lookup"
              />
              <button onClick={runBatchSearch} disabled={batchBusy} className="cv-btn cv-btn-primary text-xs">
                <Search size={12} /> {batchBusy ? "Searching..." : "Batch Search"}
              </button>
              <button onClick={runAutoCorrection} disabled={correctionBusy} className="cv-btn cv-btn-secondary text-xs">
                <Wand2 size={12} /> {correctionBusy ? "Correcting..." : "Auto Correct"}
              </button>
            </div>

            {batchResults.length > 0 && (
              <div className="glass-panel-2 p-3 rounded-lg space-y-1 max-h-48 overflow-y-auto">
                {batchResults.map((result: any, idx: number) => (
                  <div key={`${result.provider}-${idx}`} className="text-xs flex items-center justify-between">
                    <span className="font-semibold">{result.provider}</span>
                    <span className={result.ok ? "text-green-500" : "text-cv-danger"}>{result.ok ? "OK" : result.error || "Failed"}</span>
                  </div>
                ))}
              </div>
            )}

            <div className="border-t border-white/10 pt-4 space-y-3">
              <h4 className="text-xs uppercase tracking-wider text-cv-accent">Manual Replacement</h4>
              <div>
                <label className="section-label">Target Item</label>
                <select
                  value={replacement.media_id || ""}
                  onChange={(e) => {
                    const id = Number(e.target.value);
                    const item = unverifiedMedia.find((m) => m.id === id);
                    if (!item) return;
                    setReplacement({
                      media_id: id,
                      title: item.title || "",
                      year: item.year ? String(item.year) : "",
                      rating: item.rating ? String(item.rating) : "",
                      overview: item.overview || "",
                      genre: item.genre || "",
                      tmdb_id: item.tmdb_id || "",
                      imdb_id: item.imdb_id || "",
                    });
                  }}
                  className="cv-select w-full"
                >
                  <option value="">Select media item</option>
                  {unverifiedMedia.map((item) => (
                    <option key={item.id} value={item.id}>{item.title}</option>
                  ))}
                </select>
              </div>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="section-label">Title</label>
                  <input className="cv-input" value={replacement.title} onChange={(e) => setReplacement({ ...replacement, title: e.target.value })} />
                </div>
                <div>
                  <label className="section-label">Year</label>
                  <input className="cv-input" value={replacement.year} onChange={(e) => setReplacement({ ...replacement, year: e.target.value })} />
                </div>
                <div>
                  <label className="section-label">Rating</label>
                  <input className="cv-input" value={replacement.rating} onChange={(e) => setReplacement({ ...replacement, rating: e.target.value })} />
                </div>
                <div>
                  <label className="section-label">Genre</label>
                  <input className="cv-input" value={replacement.genre} onChange={(e) => setReplacement({ ...replacement, genre: e.target.value })} />
                </div>
                <div>
                  <label className="section-label">TMDb ID</label>
                  <input className="cv-input" value={replacement.tmdb_id} onChange={(e) => setReplacement({ ...replacement, tmdb_id: e.target.value })} />
                </div>
                <div>
                  <label className="section-label">IMDb ID</label>
                  <input className="cv-input" value={replacement.imdb_id} onChange={(e) => setReplacement({ ...replacement, imdb_id: e.target.value })} />
                </div>
              </div>
              <div>
                <label className="section-label">Overview</label>
                <textarea className="cv-input min-h-20" value={replacement.overview} onChange={(e) => setReplacement({ ...replacement, overview: e.target.value })} />
              </div>
              <button onClick={applyManualReplacement} className="cv-btn cv-btn-primary text-xs">
                <Save size={12} /> Save Replacement
              </button>
            </div>
          </div>
        </>
      ) : (
        <>
          <div className="glass-panel p-5">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-sm font-bold flex items-center gap-2">
                <Puzzle size={16} className="text-cv-accent" /> Plugin Repositories
              </h3>
              <button onClick={() => setShowAddRepo(!showAddRepo)} className="cv-btn cv-btn-primary text-xs">
                <Plus size={12} /> Add Repo
              </button>
            </div>
            {showAddRepo && (
              <motion.div initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} className="mb-4 glass-panel-2 p-4 rounded-lg">
                <div className="grid grid-cols-2 gap-3">
                  <div><label className="section-label">Name</label><input value={newRepo.name} onChange={(e) => setNewRepo({ ...newRepo, name: e.target.value })} className="cv-input" /></div>
                  <div><label className="section-label">Manifest URL</label><input value={newRepo.url} onChange={(e) => setNewRepo({ ...newRepo, url: e.target.value })} className="cv-input" placeholder="https://repo.example.com/manifest.json" /></div>
                </div>
                <div className="flex gap-2 mt-3">
                  <button onClick={addRepo} className="cv-btn cv-btn-primary text-xs">Save</button>
                  <button onClick={() => setShowAddRepo(false)} className="cv-btn cv-btn-secondary text-xs">Cancel</button>
                </div>
              </motion.div>
            )}
            {repos.length === 0 ? (
              <div className="text-center py-6 text-cv-subtext text-sm">No plugin repositories configured</div>
            ) : (
              <div className="space-y-2">
                {repos.map((repo) => (
                  <div key={repo.id} className="glass-panel-2 p-3 rounded-lg flex items-center justify-between">
                    <div>
                      <div className="text-sm font-semibold">{repo.name}</div>
                      <div className="text-[10px] text-cv-subtext truncate max-w-xs">{repo.url}</div>
                    </div>
                    <div className="flex gap-1">
                      <button onClick={() => syncRepo(repo.id)} className="cv-btn cv-btn-secondary text-[10px] py-1 px-2"><RefreshCw size={10} /> Sync</button>
                      <button onClick={() => loadPlugins(repo.id)} className="cv-btn cv-btn-secondary text-[10px] py-1 px-2"><Search size={10} /> Browse</button>
                      <button onClick={() => removeRepo(repo.id)} className="cv-btn cv-btn-danger text-[10px] py-1 px-2"><Trash2 size={10} /> Remove</button>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>

          {plugins.length > 0 && (
            <div className="glass-panel rounded-xl overflow-hidden">
              <div className="px-5 py-3 border-b border-white/5"><h3 className="text-sm font-bold">Plugin Catalog ({plugins.length})</h3></div>
              <div className="divide-y divide-white/5 max-h-80 overflow-y-auto">
                {plugins.map((plugin, i) => (
                  <div key={plugin.id || i} className="px-5 py-3 flex items-center justify-between">
                    <div>
                      <div className="text-sm font-semibold">{plugin.name} <span className="text-[10px] text-cv-subtext">{plugin.version}</span></div>
                      <div className="text-[10px] text-cv-subtext">{plugin.description || "No description"} - {plugin.author || "Unknown author"}</div>
                    </div>
                    <button onClick={() => installPlugin(plugin.id)} className={`cv-btn ${plugin.installed ? "cv-btn-secondary" : "cv-btn-primary"} text-xs`}>
                      {plugin.installed ? <><CheckCircle size={12} /> Installed</> : <><Download size={12} /> Install</>}
                    </button>
                  </div>
                ))}
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}

const DEMO_REPOS = [{ id: 1, name: "Jellyfin Official", url: "https://repo.jellyfin.org/releases/plugin/manifest.json", enabled: true, last_synced: null }];
const DEMO_PLUGINS = [
  { id: 1, name: "TMDb Plugin", version: "1.0", description: "Fetch metadata from TMDb", author: "Jellyfin", installed: true },
  { id: 2, name: "OpenSubtitles", version: "2.1", description: "Auto-download subtitles", author: "Community", installed: false },
];
