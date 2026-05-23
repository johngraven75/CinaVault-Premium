// CinaVault Premium — Plugins & Metadata Tab (Full Plugin Registry + Metadata Submenu)
import React, { useState, useMemo, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "../../store/appStore";
import {
  FULL_PLUGIN_REGISTRY,
  PluginEntry, PluginCategory, PluginPlatform, PluginStatus,
} from "../../data/pluginRegistry";
import { pluginEngine } from "../../data/pluginAdapter";
import { applyPluginRuntimeState, getMetadataProviderInitials, matchesPluginSearch } from "../../utils/pluginUiSafety";
import {
  Package, Search, Filter, Download, Trash2, Settings, Play,
  CheckCircle2, XCircle, RefreshCw, ChevronDown, ChevronRight,
  Layers, Clock, ToggleLeft, ToggleRight, AlertTriangle,
  Database, Image, Film, Tag, Zap, Key, TestTube2,
} from "lucide-react";

type SubView = "plugins" | "metadata" | "tasks";

const CATEGORY_LABELS: Record<PluginCategory, string> = {
  metadata: "Metadata Providers", subtitles: "Subtitles", live_tv: "Live TV & DVR",
  notifications: "Notifications", management: "Library Management", channels: "Channels & Content",
  social: "Social & Watch Party", stats: "Statistics & Reports", artwork: "Artwork & Images",
  utilities: "Utilities & Tools", themes: "Themes & UI", content_providers: "Content Providers",
  playback: "Playback Enhancements", sync: "Sync & Scrobbling", security: "Security & Auth",
  ai: "AI & Machine Learning",
};

const PLATFORM_COLORS: Record<PluginPlatform, string> = {
  jellyfin: "#a78bfa", emby: "#52b54b", plex: "#e5a00d", cinavault: "#60a5fa",
};

const PLATFORM_LABELS: Record<PluginPlatform, string> = {
  jellyfin: "MS-C", emby: "MS-B", plex: "MS-A", cinavault: "CinaVault",
};

const PLATFORM_FILTER_OPTIONS: PluginPlatform[] = ["jellyfin", "emby", "plex", "cinavault"];

const TASK_FREQ_OPTIONS = [
  { value: "manual", label: "Manual Only" },
  { value: "on_scan", label: "On Library Scan" },
  { value: "on_import", label: "On Media Import" },
  { value: "daily", label: "Daily" },
  { value: "weekly", label: "Weekly" },
  { value: "never", label: "Never" },
];

const KEYED_METADATA_PROVIDERS = new Set([
  "tmdb", "omdb", "tvdb", "fanart", "audiodb", "tpdb", "stashdb",
  "anidb", "mal", "igdb", "goodreads", "lastfm", "discogs",
  "trakt", "opensubtitles",
]);

export default function PluginsTab() {
  const {
    metadataProviders, toggleMetadataProvider, enableAllProviders, disableAllProviders,
    scheduledTasks, setTaskFrequency, addStatusMessage,
  } = useAppStore();

  const [subView, setSubView] = useState<SubView>("plugins");
  const [search, setSearch] = useState("");
  const [platformFilter, setPlatformFilter] = useState<PluginPlatform | "all">("all");
  const [categoryFilter, setCategoryFilter] = useState<PluginCategory | "all">("all");
  const [plugins, setPlugins] = useState<PluginEntry[]>(FULL_PLUGIN_REGISTRY);
  const [expandedCategories, setExpandedCategories] = useState<Set<string>>(new Set(["metadata", "subtitles", "sync", "management"]));
  const [installing, setInstalling] = useState<Set<string>>(new Set());
  const [metaCategoryFilter, setMetaCategoryFilter] = useState<string>("all");
  const [apiKeys, setApiKeys] = useState<Record<string, string>>({});
  const [editingKey, setEditingKey] = useState<string | null>(null);
  const [keyValue, setKeyValue] = useState("");
  const [testingKey, setTestingKey] = useState<string | null>(null);

  const statusForPlugin = (plugin: PluginEntry, enabled = true): PluginStatus =>
    enabled ? (plugin.cinavaultNative ? "active" : "installed") : "disabled";

  // Load installed status on mount
  useEffect(() => {
    pluginEngine.loadFromBackend().then(() => {
      setPlugins(applyPluginRuntimeState(FULL_PLUGIN_REGISTRY, pluginEngine.getInstalled()));
      addStatusMessage(`Plugin catalog loaded (${pluginEngine.getInstalled().length} installed)`);
    });
  }, [addStatusMessage]);

  useEffect(() => {
    invoke<Record<string, string>>("get_api_keys")
      .then(setApiKeys)
      .catch(() => setApiKeys({}));
  }, []);

  // Filtered plugins
  const filtered = useMemo(() => {
    return plugins.filter(p => {
      if (!matchesPluginSearch(p, search)) return false;
      if (platformFilter !== "all" && !p.platforms.includes(platformFilter)) return false;
      if (categoryFilter !== "all" && p.category !== categoryFilter) return false;
      return true;
    });
  }, [plugins, search, platformFilter, categoryFilter]);

  // Group by category
  const grouped = useMemo(() => {
    const map = new Map<PluginCategory, PluginEntry[]>();
    for (const p of filtered) {
      const list = map.get(p.category) || [];
      list.push(p);
      map.set(p.category, list);
    }
    return map;
  }, [filtered]);

  // Metadata providers grouped by category
  const metaGroups = useMemo(() => {
    const map = new Map<string, typeof metadataProviders>();
    for (const p of metadataProviders) {
      if (metaCategoryFilter !== "all" && p.category !== metaCategoryFilter) continue;
      const list = map.get(p.category) || [];
      list.push(p);
      map.set(p.category, list);
    }
    return map;
  }, [metadataProviders, metaCategoryFilter]);

  const metaCategories = useMemo(() => {
    return [...new Set(metadataProviders.map(p => p.category))];
  }, [metadataProviders]);

  // Install handler
  const handleInstall = async (plugin: PluginEntry) => {
    setInstalling(prev => new Set(prev).add(plugin.id));
    const success = await pluginEngine.installPlugin(plugin);
    if (success) {
      setPlugins(prev => prev.map(p =>
        p.id === plugin.id ? { ...p, status: statusForPlugin(plugin) } : p
      ));
      addStatusMessage(`Downloaded and enabled: ${plugin.name} (${plugin.platforms.map(p => PLATFORM_LABELS[p]).join(", ")})`);
    }
    setInstalling(prev => { const s = new Set(prev); s.delete(plugin.id); return s; });
  };

  // Uninstall handler
  const handleUninstall = async (plugin: PluginEntry) => {
    await pluginEngine.uninstallPlugin(plugin.id);
    setPlugins(prev => prev.map(p =>
      p.id === plugin.id ? { ...p, status: "available" as PluginStatus } : p
    ));
    addStatusMessage(`Uninstalled: ${plugin.name}`);
  };

  const handleSetEnabled = async (plugin: PluginEntry, enabled: boolean) => {
    if (!pluginEngine.getInstalledPlugin(plugin.id)) {
      await pluginEngine.installPlugin(plugin);
    }
    await pluginEngine.setPluginEnabled(plugin.id, enabled);
    setPlugins(prev => prev.map(p =>
      p.id === plugin.id ? { ...p, status: statusForPlugin(plugin, enabled) } : p
    ));
    addStatusMessage(`${plugin.name}: ${enabled ? "enabled" : "disabled"}`);
  };

  const toggleCategory = (cat: string) => {
    setExpandedCategories(prev => {
      const s = new Set(prev);
      s.has(cat) ? s.delete(cat) : s.add(cat);
      return s;
    });
  };

  const enabledProviderIds = (providers = metadataProviders) =>
    providers.filter((provider) => provider.enabled).map((provider) => provider.id);

  const persistProviderSelection = async (providerIds: string[]) => {
    const value = JSON.stringify(providerIds);
    try {
      await invoke("set_setting", { key: "metadata_selected_providers", value });
    } catch (error) {
      addStatusMessage(`Provider selection save failed: ${error}`);
    }
  };

  const toggleProviderAndPersist = async (id: string) => {
    const nextProviders = metadataProviders.map((provider) =>
      provider.id === id ? { ...provider, enabled: !provider.enabled } : provider,
    );
    toggleMetadataProvider(id);
    await persistProviderSelection(enabledProviderIds(nextProviders));
  };

  const setProvidersForCategory = async (enabled: boolean) => {
    const category = metaCategoryFilter === "all" ? undefined : metaCategoryFilter;
    const nextProviders = metadataProviders.map((provider) =>
      !category || provider.category === category ? { ...provider, enabled } : provider,
    );
    if (enabled) {
      enableAllProviders(category);
    } else {
      disableAllProviders(category);
    }
    await persistProviderSelection(enabledProviderIds(nextProviders));
  };

  const saveApiKey = async (providerId: string) => {
    const trimmed = keyValue.trim();
    if (!trimmed) {
      addStatusMessage(`API key is empty for ${providerId}`);
      return;
    }
    try {
      await invoke("set_api_key", { provider: providerId, apiKey: trimmed });
      const refreshed = await invoke<Record<string, string>>("get_api_keys");
      setApiKeys(refreshed);
      setEditingKey(null);
      setKeyValue("");
      addStatusMessage(`API key saved for ${providerId}`);
    } catch (error) {
      addStatusMessage(`API key save failed for ${providerId}: ${error}`);
    }
  };

  const testApiKey = async (providerId: string) => {
    const candidate = keyValue.trim();
    if (!candidate) {
      addStatusMessage(`Paste an API key before testing ${providerId}`);
      return;
    }
    setTestingKey(providerId);
    try {
      const result = await invoke<{ valid?: boolean }>("test_api_key", {
        provider: providerId,
        apiKey: candidate,
      });
      addStatusMessage(`${providerId} API key test: ${result.valid ? "valid" : "invalid"}`);
    } catch (error) {
      addStatusMessage(`${providerId} API key test failed: ${error}`);
    } finally {
      setTestingKey(null);
    }
  };

  const installedCount = plugins.filter(p => p.status === "installed" || p.status === "active").length;
  const totalCount = plugins.length;

  return (
    <div className="space-y-5">
      {/* ── Sub-navigation ── */}
      <div className="flex items-center gap-2 flex-wrap">
        {([
          { id: "plugins" as SubView, label: "Plugin Catalog", icon: Package, count: totalCount },
          { id: "metadata" as SubView, label: "Metadata Providers", icon: Database, count: metadataProviders.length },
          { id: "tasks" as SubView, label: "Scheduled Tasks", icon: Clock, count: 4 },
        ]).map(tab => (
          <button
            key={tab.id}
            onClick={() => setSubView(tab.id)}
            className={`flex items-center gap-2 px-4 py-2.5 rounded-xl text-sm font-medium transition-all ${
              subView === tab.id
                ? "bg-gradient-to-r from-[var(--cv-accent)]/30 to-[var(--cv-accent)]/10 text-[var(--cv-accent)] border border-[var(--cv-accent)]/30"
                : "bg-white/5 hover:bg-white/10 text-[var(--cv-subtext)]"
            }`}
          >
            <tab.icon size={16} />
            {tab.label}
            <span className="text-xs px-1.5 py-0.5 rounded-full bg-white/10">{tab.count}</span>
          </button>
        ))}
      </div>

      <AnimatePresence mode="wait">
        {/* ════════════════════════════════════════════════════════════ */}
        {/*  PLUGIN CATALOG                                            */}
        {/* ════════════════════════════════════════════════════════════ */}
        {subView === "plugins" && (
          <motion.div key="plugins" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="space-y-4">
            {/* Stats Bar */}
            <div className="grid grid-cols-4 gap-3">
              {[
                { label: "Total Plugins", value: totalCount, icon: Package, color: "var(--cv-accent)" },
                { label: "Installed", value: installedCount, icon: CheckCircle2, color: "#22c55e" },
                { label: "MS-C", value: plugins.filter(p => p.platforms.includes("jellyfin")).length, icon: Layers, color: "#a78bfa" },
                { label: "MS-B", value: plugins.filter(p => p.platforms.includes("emby")).length, icon: Layers, color: "#52b54b" },
              ].map((s, i) => (
                <div key={i} className="cv-card p-3 flex items-center gap-3">
                  <div className="w-9 h-9 rounded-lg flex items-center justify-center" style={{ background: `${s.color}22` }}>
                    <s.icon size={18} style={{ color: s.color }} />
                  </div>
                  <div>
                    <div className="text-lg font-bold" style={{ color: "var(--cv-text)" }}>{s.value}</div>
                    <div className="text-[10px]" style={{ color: "var(--cv-subtext)" }}>{s.label}</div>
                  </div>
                </div>
              ))}
            </div>

            {/* Filters */}
            <div className="flex items-center gap-3 flex-wrap">
              <div className="relative flex-1 min-w-[200px]">
                <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-[var(--cv-subtext)]" />
                <input type="text" value={search} onChange={e => setSearch(e.target.value)}
                  placeholder="Search plugins by name, tag, or description..."
                  className="cv-input pl-9 w-full text-xs" />
              </div>
              <select value={platformFilter} onChange={e => setPlatformFilter(e.target.value as any)}
                className="cv-input text-xs min-w-[130px]">
                <option value="all">All Platforms</option>
                {PLATFORM_FILTER_OPTIONS.map(p => (
                  <option key={p} value={p}>{PLATFORM_LABELS[p]}</option>
                ))}
              </select>
              <select value={categoryFilter} onChange={e => setCategoryFilter(e.target.value as any)}
                className="cv-input text-xs min-w-[160px]">
                <option value="all">All Categories</option>
                {Object.entries(CATEGORY_LABELS).map(([k,v]) => (
                  <option key={k} value={k}>{v}</option>
                ))}
              </select>
            </div>

            {/* Grouped Plugin List */}
            <div className="space-y-3">
              {Array.from(grouped.entries()).map(([cat, items]) => (
                <div key={cat} className="cv-card overflow-hidden">
                  <button onClick={() => toggleCategory(cat)}
                    className="w-full flex items-center justify-between p-3 hover:bg-white/5 transition-colors">
                    <div className="flex items-center gap-2">
                      {expandedCategories.has(cat) ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
                      <span className="font-semibold text-sm" style={{ color: "var(--cv-text)" }}>
                        {CATEGORY_LABELS[cat] || cat}
                      </span>
                      <span className="text-[10px] px-2 py-0.5 rounded-full bg-white/10 text-[var(--cv-subtext)]">
                        {items.length} plugins
                      </span>
                    </div>
                  </button>

                  <AnimatePresence>
                    {expandedCategories.has(cat) && (
                      <motion.div initial={{ height: 0, opacity: 0 }} animate={{ height: "auto", opacity: 1 }}
                        exit={{ height: 0, opacity: 0 }} transition={{ duration: 0.2 }}
                        className="border-t border-white/5">
                        {items.map(plugin => (
                          <PluginRow key={plugin.id} plugin={plugin}
                            installing={installing.has(plugin.id)}
                            onInstall={() => handleInstall(plugin)}
                            onUninstall={() => handleUninstall(plugin)}
                            onSetEnabled={(enabled) => handleSetEnabled(plugin, enabled)} />
                        ))}
                      </motion.div>
                    )}
                  </AnimatePresence>
                </div>
              ))}
            </div>

            {filtered.length === 0 && (
              <div className="cv-card p-8 text-center">
                <Package size={40} className="mx-auto mb-3 opacity-30" />
                <p className="text-sm" style={{ color: "var(--cv-subtext)" }}>No plugins match your filters</p>
              </div>
            )}
          </motion.div>
        )}

        {/* ════════════════════════════════════════════════════════════ */}
        {/*  METADATA PROVIDERS SUBMENU                                */}
        {/* ════════════════════════════════════════════════════════════ */}
        {subView === "metadata" && (
          <motion.div key="metadata" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="space-y-4">
            <div className="cv-card p-4">
              <div className="flex items-center justify-between mb-4">
                <div>
                  <h3 className="text-base font-bold" style={{ color: "var(--cv-text)" }}>Metadata Providers</h3>
                  <p className="text-xs mt-0.5" style={{ color: "var(--cv-subtext)" }}>
                    Select which metadata providers to use for scanning. Enabled: {metadataProviders.filter(p => p.enabled).length} / {metadataProviders.length}
                  </p>
                </div>
                <div className="flex items-center gap-2">
                  <select value={metaCategoryFilter} onChange={e => setMetaCategoryFilter(e.target.value)}
                    className="cv-input text-xs">
                    <option value="all">All Categories</option>
                    {metaCategories.map(c => <option key={c} value={c}>{c}</option>)}
                  </select>
                  <button onClick={() => void setProvidersForCategory(true)}
                    className="cv-btn-sm text-xs bg-green-500/20 text-green-400 hover:bg-green-500/30">
                    Enable All
                  </button>
                  <button onClick={() => void setProvidersForCategory(false)}
                    className="cv-btn-sm text-xs bg-red-500/20 text-red-400 hover:bg-red-500/30">
                    Disable All
                  </button>
                </div>
              </div>

              {/* Provider groups */}
              {Array.from(metaGroups.entries()).map(([category, providers]) => (
                <div key={category} className="mb-4">
                  <h4 className="text-xs font-semibold uppercase tracking-wider mb-2 px-1"
                    style={{ color: "var(--cv-accent)" }}>{category}</h4>
                  <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-2">
                    {providers.map(provider => {
                      const hasApiKey = Boolean(apiKeys[provider.id]);
                      const needsApiKey = KEYED_METADATA_PROVIDERS.has(provider.id);
                      const isEditing = editingKey === provider.id;
                      return (
                      <div key={provider.id}
                        className={`flex flex-col gap-2 p-3 rounded-xl border transition-all ${
                          provider.enabled
                            ? "border-[var(--cv-accent)]/40 bg-[var(--cv-accent)]/10"
                            : "border-white/5 bg-white/3 hover:bg-white/5"
                        }`}>
                        <button onClick={() => void toggleProviderAndPersist(provider.id)} className="flex items-center gap-3 text-left">
                          <div className={`w-8 h-8 rounded-lg flex items-center justify-center text-xs font-bold ${
                            provider.enabled ? "bg-[var(--cv-accent)]/30 text-[var(--cv-accent)]" : "bg-white/10 text-[var(--cv-subtext)]"
                          }`}>
                            {getMetadataProviderInitials(provider.name)}
                          </div>
                          <div className="flex-1 text-left">
                            <div className="text-xs font-medium" style={{ color: provider.enabled ? "var(--cv-text)" : "var(--cv-subtext)" }}>
                              {provider.name}
                            </div>
                            <div className="text-[10px]" style={{ color: "var(--cv-subtext)" }}>{provider.id} · {provider.category}</div>
                          </div>
                          {provider.enabled
                            ? <ToggleRight size={20} className="text-[var(--cv-accent)]" />
                            : <ToggleLeft size={20} className="text-[var(--cv-subtext)]/40" />
                          }
                        </button>
                        {needsApiKey && (
                          <div className="pt-2 border-t border-white/5">
                            {isEditing ? (
                              <div className="flex gap-2">
                                <input
                                  value={keyValue}
                                  onChange={(e) => setKeyValue(e.target.value)}
                                  type="password"
                                  className="cv-input text-xs flex-1 min-w-0"
                                  placeholder={`${provider.name} API key`}
                                />
                                <button onClick={() => void saveApiKey(provider.id)} className="cv-btn-sm text-xs bg-[var(--cv-accent)]/20 text-[var(--cv-accent)]">
                                  <Key size={12} /> Save
                                </button>
                                <button onClick={() => void testApiKey(provider.id)} disabled={testingKey === provider.id} className="cv-btn-sm text-xs bg-white/10 text-[var(--cv-subtext)]">
                                  <TestTube2 size={12} /> Test
                                </button>
                              </div>
                            ) : (
                              <div className="flex items-center justify-between gap-2">
                                <span className="text-[10px]" style={{ color: hasApiKey ? "var(--cv-accent)" : "var(--cv-subtext)" }}>
                                  {hasApiKey ? `Key set (${apiKeys[provider.id]})` : "API key not set"}
                                </span>
                                <button
                                  onClick={() => { setEditingKey(provider.id); setKeyValue(""); }}
                                  className="cv-btn-sm text-xs bg-white/10 text-[var(--cv-subtext)]"
                                >
                                  <Key size={12} /> {hasApiKey ? "Update" : "Set Key"}
                                </button>
                              </div>
                            )}
                          </div>
                        )}
                        </div>
                      );
                    })}
                  </div>
                </div>
              ))}
            </div>
          </motion.div>
        )}

        {/* ════════════════════════════════════════════════════════════ */}
        {/*  SCHEDULED TASKS                                           */}
        {/* ════════════════════════════════════════════════════════════ */}
        {subView === "tasks" && (
          <motion.div key="tasks" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="space-y-4">
            <div className="cv-card p-4">
              <h3 className="text-base font-bold mb-1" style={{ color: "var(--cv-text)" }}>Scheduled Tasks</h3>
              <p className="text-xs mb-4" style={{ color: "var(--cv-subtext)" }}>
                Configure when each task runs automatically. Changes are saved and persisted on exit.
              </p>

              <div className="space-y-3">
                {([
                  { key: "thumbnails" as const, label: "Generate Thumbnails", desc: "Create preview thumbnails for all media items", icon: Image, color: "#a78bfa" },
                  { key: "chapter_images" as const, label: "Generate Chapter Images", desc: "Extract chapter preview images from video files", icon: Film, color: "#22c55e" },
                  { key: "metadata_check" as const, label: "Metadata Check", desc: "Scan for missing or outdated metadata across all libraries", icon: Tag, color: "#f59e0b" },
                  { key: "match_unmatch" as const, label: "Match / Unmatch Media", desc: "Auto-match unidentified media and flag mismatches", icon: Zap, color: "#ef4444" },
                ]).map(task => (
                  <div key={task.key} className="flex items-center gap-4 p-4 rounded-xl border border-white/5 bg-white/3 hover:bg-white/5 transition-colors">
                    <div className="w-10 h-10 rounded-xl flex items-center justify-center shrink-0" style={{ background: `${task.color}22` }}>
                      <task.icon size={20} style={{ color: task.color }} />
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="text-sm font-semibold" style={{ color: "var(--cv-text)" }}>{task.label}</div>
                      <div className="text-[11px]" style={{ color: "var(--cv-subtext)" }}>{task.desc}</div>
                    </div>
                    <select
                      value={scheduledTasks[task.key]}
                      onChange={e => {
                        setTaskFrequency(task.key, e.target.value as any);
                        addStatusMessage(`Task "${task.label}" set to: ${e.target.value}`);
                      }}
                      className="cv-input text-xs min-w-[150px]"
                    >
                      {TASK_FREQ_OPTIONS.map(opt => (
                        <option key={opt.value} value={opt.value}>{opt.label}</option>
                      ))}
                    </select>
                  </div>
                ))}
              </div>
            </div>

            {/* Quick-run buttons */}
            <div className="cv-card p-4">
              <h4 className="text-sm font-semibold mb-3" style={{ color: "var(--cv-text)" }}>Run Now</h4>
              <div className="grid grid-cols-2 lg:grid-cols-4 gap-2">
                {[
                  { label: "Thumbnails", action: "generate_thumbnails" },
                  { label: "Chapter Images", action: "generate_chapters" },
                  { label: "Metadata Check", action: "check_metadata" },
                  { label: "Match/Unmatch", action: "match_media" },
                ].map(btn => (
                  <button key={btn.action} onClick={async () => {
                    addStatusMessage(`Running: ${btn.label}...`);
                    try { await invoke("run_plugin", { pluginId: btn.action, action: "run" }); } catch {}
                    addStatusMessage(`Completed: ${btn.label}`);
                  }}
                  className="cv-btn text-xs py-2.5 flex items-center justify-center gap-2">
                    <Play size={12} /> {btn.label}
                  </button>
                ))}
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

// ── Plugin Row Component ──
function PluginRow({ plugin, installing, onInstall, onUninstall, onSetEnabled }: {
  plugin: PluginEntry; installing: boolean; onInstall: () => void; onUninstall: () => void; onSetEnabled: (enabled: boolean) => void;
}) {
  const [showConfig, setShowConfig] = useState(false);
  const [configText, setConfigText] = useState("{}");
  const [configError, setConfigError] = useState("");
  const compat = pluginEngine.checkCompatibility(plugin);
  const isInstalled = plugin.status === "installed" || plugin.status === "active" || plugin.status === "disabled";
  const enabled = isInstalled && plugin.status !== "disabled" && pluginEngine.getInstalledPlugin(plugin.id)?.enabled !== false;

  useEffect(() => {
    if (!showConfig) return;
    setConfigText(JSON.stringify(pluginEngine.getPluginConfig(plugin.id), null, 2));
    setConfigError("");
  }, [plugin.id, showConfig]);

  const saveConfig = async () => {
    try {
      const parsed = JSON.parse(configText || "{}");
      if (!pluginEngine.getInstalledPlugin(plugin.id)) {
        await pluginEngine.installPlugin(plugin);
      }
      await pluginEngine.setPluginConfig(plugin.id, parsed);
      setConfigError("");
    } catch (error) {
      setConfigError(`Invalid JSON: ${error}`);
    }
  };

  const runPlugin = async () => {
    if (!pluginEngine.getInstalledPlugin(plugin.id)) {
      await pluginEngine.installPlugin(plugin);
    }
    await pluginEngine.runPlugin(plugin.id, "run");
  };

  return (
    <div className="border-b border-white/3 last:border-b-0">
      <div className="flex items-center gap-3 px-4 py-3 hover:bg-white/3 transition-colors">
        {/* Icon */}
        <div className="w-9 h-9 rounded-lg flex items-center justify-center text-base bg-white/5 shrink-0">
          {plugin.icon}
        </div>

        {/* Info */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium truncate" style={{ color: "var(--cv-text)" }}>{plugin.name}</span>
            {plugin.platforms.map(p => (
              <span key={p} className="text-[9px] font-bold px-1.5 py-0.5 rounded-full"
                style={{ background: `${PLATFORM_COLORS[p]}22`, color: PLATFORM_COLORS[p] }}>
                {PLATFORM_LABELS[p]}
              </span>
            ))}
            {plugin.premium && (
              <span className="text-[9px] font-bold px-1.5 py-0.5 rounded-full bg-amber-500/20 text-amber-400">PREMIUM</span>
            )}
            {plugin.cinavaultNative && (
              <span className="text-[9px] font-bold px-1.5 py-0.5 rounded-full bg-blue-500/20 text-blue-400">NATIVE</span>
            )}
            {plugin.status === "disabled" && (
              <span className="text-[9px] font-bold px-1.5 py-0.5 rounded-full bg-white/10 text-[var(--cv-subtext)]">DISABLED</span>
            )}
          </div>
          <div className="text-[11px] truncate" style={{ color: "var(--cv-subtext)" }}>{plugin.description}</div>
          <div className="text-[10px] mt-0.5" style={{ color: "var(--cv-subtext)" }}>
            v{plugin.version} · {plugin.author} · {compat.reason}
          </div>
        </div>

        {/* Actions */}
        <div className="flex items-center gap-2 shrink-0">
          {plugin.configurable && isInstalled && (
            <button onClick={() => setShowConfig(!showConfig)}
              title="Configure plugin"
              className="w-7 h-7 rounded-lg flex items-center justify-center bg-white/5 hover:bg-white/10 transition-colors">
              <Settings size={13} className="text-[var(--cv-subtext)]" />
            </button>
          )}
          {isInstalled && (
            <button onClick={() => onSetEnabled(!enabled)}
              className={`cv-btn-sm text-[11px] flex items-center gap-1 ${
                enabled ? "bg-green-500/15 text-green-400 hover:bg-green-500/25" : "bg-white/10 text-[var(--cv-subtext)] hover:bg-white/15"
              }`}>
              {enabled ? <ToggleRight size={12} /> : <ToggleLeft size={12} />}
              {enabled ? "Enabled" : "Enable"}
            </button>
          )}
          {isInstalled ? (
            <button onClick={onUninstall}
              className="cv-btn-sm text-[11px] bg-red-500/15 text-red-400 hover:bg-red-500/25 flex items-center gap-1">
              <Trash2 size={11} /> Remove
            </button>
          ) : (
            <button onClick={onInstall} disabled={installing}
              className="cv-btn-sm text-[11px] bg-[var(--cv-accent)]/15 text-[var(--cv-accent)] hover:bg-[var(--cv-accent)]/25 flex items-center gap-1 disabled:opacity-50">
              {installing ? <RefreshCw size={11} className="animate-spin" /> : <Download size={11} />}
              {installing ? "Installing..." : "Download & Enable"}
            </button>
          )}
        </div>
      </div>

      <AnimatePresence>
        {showConfig && isInstalled && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            className="overflow-hidden border-t border-white/5 bg-black/15"
          >
            <div className="px-4 py-3 grid grid-cols-1 lg:grid-cols-[1fr_auto] gap-3">
              <div>
                <div className="text-[10px] uppercase tracking-wider mb-1" style={{ color: "var(--cv-subtext)" }}>
                  Plugin settings JSON
                </div>
                <textarea
                  value={configText}
                  onChange={(event) => setConfigText(event.target.value)}
                  className="cv-input w-full min-h-[84px] font-mono text-[11px]"
                  spellCheck={false}
                />
                {configError && <div className="text-[10px] text-red-400 mt-1">{configError}</div>}
              </div>
              <div className="flex lg:flex-col gap-2 lg:items-stretch">
                <button onClick={saveConfig} className="cv-btn-sm text-[11px] bg-[var(--cv-accent)]/15 text-[var(--cv-accent)] hover:bg-[var(--cv-accent)]/25">
                  Save Settings
                </button>
                <button onClick={runPlugin} className="cv-btn-sm text-[11px] bg-white/10 text-[var(--cv-subtext)] hover:bg-white/15">
                  Run Plugin
                </button>
                {plugin.repo && (
                  <button onClick={() => window.open(plugin.repo, "_blank", "noopener,noreferrer")} className="cv-btn-sm text-[11px] bg-white/10 text-[var(--cv-subtext)] hover:bg-white/15">
                    Source
                  </button>
                )}
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

