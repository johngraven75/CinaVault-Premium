// CinaVault Premium — Settings Tab (7 sub-categories)
import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence } from "framer-motion";
import { useAppStore } from "../../store/appStore";
import { applyTheme, THEME_PRESETS } from "../../themes";
import {
  Settings, Monitor, Play, Library, Link, Cloud, Copy, Zap,
  Palette, Eye, Sparkles, SkipForward, Subtitles, Film, FolderOpen,
  Gauge, Shield, Wifi, CheckCircle, Search, Trash2, SlidersHorizontal, Save
} from "lucide-react";

type SettingsSection = "interface" | "playback" | "library" | "integrations" | "cloud" | "duplicates" | "power";

const SECTIONS: { id: SettingsSection; label: string; icon: React.FC<any> }[] = [
  { id: "interface", label: "Interface", icon: Monitor },
  { id: "playback", label: "Playback", icon: Play },
  { id: "library", label: "Library", icon: Library },
  { id: "integrations", label: "Integrations", icon: Link },
  { id: "cloud", label: "Cloud Storage", icon: Cloud },
  { id: "duplicates", label: "Duplicate Finder", icon: Copy },
  { id: "power", label: "Power & Safety", icon: Zap },
];

export default function SettingsTab() {
  const { settings, setSetting, currentTheme, setTheme, addStatusMessage } = useAppStore();
  const [activeSection, setActiveSection] = useState<SettingsSection>("interface");
  const [dupMatchBy, setDupMatchBy] = useState("name_size");
  const [dupTolerance, setDupTolerance] = useState("0");
  const [dupResults, setDupResults] = useState<any>(null);
  const [dupScanning, setDupScanning] = useState(false);

  const updateSetting = async (key: string, value: string) => {
    setSetting(key, value);
    try { await invoke("set_setting", { key, value }); } catch {}
  };

  const handleThemeChange = (themeId: string) => {
    setTheme(themeId);
    applyTheme(themeId);
    updateSetting("theme", themeId);
    addStatusMessage(`Theme changed to ${THEME_PRESETS.find(t => t.id === themeId)?.name}`);
  };

  const [selectedDuplicates, setSelectedDuplicates] = useState<number[]>([]);

  const runDuplicateScan = async () => {
    setDupScanning(true);
    addStatusMessage("Scanning for duplicates...");
    try {
      const result = await invoke<any>("find_duplicates", { matchBy: dupMatchBy, toleranceMb: parseFloat(dupTolerance) || 0 });
      setDupResults(result);
      setSelectedDuplicates([]); // Clear selection on new scan
      addStatusMessage(`Found ${result.groups_found} duplicate groups (${result.total_duplicates} items)`);
    } catch (e) { addStatusMessage(`Scan failed: ${e}`); }
    setDupScanning(false);
  };

  const toggleDuplicateSelection = (itemId: number) => {
    setSelectedDuplicates(prev => 
      prev.includes(itemId)
        ? prev.filter(id => id !== itemId)
        : [...prev, itemId]
    );
  };

  const selectAllDuplicates = () => {
    if (dupResults && dupResults.groups) {
      // Collect all item IDs from all groups
      const allItemIds = dupResults.groups.flatMap((group: any) => 
        group.items.map((item: any) => item.id)
      );
      setSelectedDuplicates(allItemIds);
      addStatusMessage(`Selected all ${allItemIds.length} duplicate items`);
    } else if (dupResults && dupResults.total_duplicates > 0) {
      addStatusMessage("Duplicate groups data not available for selection");
    } else {
      addStatusMessage("No duplicates to select");
    }
  };

  const deleteSelectedDuplicates = async () => {
    if (selectedDuplicates.length === 0) {
      addStatusMessage("No duplicates selected for deletion");
      return;
    }

    if (!window.confirm(`Delete ${selectedDuplicates.length} selected duplicate items? This action cannot be undone.`)) {
      return;
    }

    try {
      // Delete each selected duplicate
      for (const itemId of selectedDuplicates) {
        await invoke("remove_duplicate", { id: itemId, delete_file: true });
      }
      
      addStatusMessage(`Deleted ${selectedDuplicates.length} duplicate items`);
      setSelectedDuplicates([]);
      
      // Rescan to update results
      runDuplicateScan();
    } catch (e) {
      addStatusMessage(`Delete failed: ${e}`);
    }
  };

  const ToggleSetting = ({ settingKey, label, desc }: { settingKey: string; label: string; desc?: string }) => {
    const enabled = settings[settingKey] === "true";
    return (
      <div className="flex items-center justify-between py-2.5 px-3 rounded hover:bg-white/[0.02]">
        <div>
          <div className="text-sm">{label}</div>
          {desc && <div className="text-[10px] text-cv-subtext">{desc}</div>}
        </div>
        <div className={`cv-toggle ${enabled ? "active" : ""}`} onClick={() => updateSetting(settingKey, enabled ? "false" : "true")} />
      </div>
    );
  };

  return (
    <div className="flex gap-5 h-full">
      {/* Settings Sidebar */}
      <div className="w-48 shrink-0 space-y-1">
        {SECTIONS.map(section => (
          <button
            key={section.id}
            onClick={() => setActiveSection(section.id)}
            className={`w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-left text-sm transition-all ${
              activeSection === section.id
                ? "bg-cv-accent/15 text-cv-accent font-semibold"
                : "text-cv-subtext hover:text-cv-text hover:bg-white/5"
            }`}
          >
            <section.icon size={14} />
            {section.label}
          </button>
        ))}
      </div>

      {/* Settings Content */}
      <div className="flex-1 min-w-0">
        <AnimatePresence mode="wait">
          <motion.div
            key={activeSection}
            initial={{ opacity: 0, x: 12 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -12 }}
            transition={{ duration: 0.2 }}
            className="space-y-4"
          >
            {/* Interface */}
            {activeSection === "interface" && (
              <>
                <div className="glass-panel p-5">
                  <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
                    <Palette size={16} className="text-cv-accent" /> Theme Preset
                  </h3>
                  <div className="grid grid-cols-2 md:grid-cols-3 gap-3">
                    {THEME_PRESETS.map(theme => (
                      <button
                        key={theme.id}
                        onClick={() => handleThemeChange(theme.id)}
                        className={`glass-panel-2 p-3 rounded-lg text-left transition-all ${
                          currentTheme === theme.id ? "ring-2 ring-cv-accent" : "hover:bg-white/5"
                        }`}
                      >
                        <div className="flex gap-1.5 mb-2">
                          {["accent", "neon-1", "neon-2", "neon-3"].map(key => (
                            <div key={key} className="w-4 h-4 rounded-full" style={{ background: theme.colors[key] }} />
                          ))}
                        </div>
                        <div className="text-xs font-semibold">{theme.name}</div>
                        <div className="text-[9px] text-cv-subtext">{theme.id}</div>
                      </button>
                    ))}
                  </div>
                </div>
                <div className="glass-panel p-5 space-y-1">
                  <h3 className="text-sm font-bold mb-3 flex items-center gap-2">
                    <Monitor size={16} className="text-cv-accent" /> Window & Display
                  </h3>
                  <div className="grid grid-cols-2 gap-3 mb-3">
                    <div>
                      <label className="section-label">Window Width</label>
                      <input value={settings.window_width || "1400"} onChange={e => updateSetting("window_width", e.target.value)} className="cv-input" />
                    </div>
                    <div>
                      <label className="section-label">Window Height</label>
                      <input value={settings.window_height || "900"} onChange={e => updateSetting("window_height", e.target.value)} className="cv-input" />
                    </div>
                  </div>
                  <div className="mb-3">
                    <label className="section-label">Window Opacity (%)</label>
                    <input type="range" min="50" max="100" value={settings.window_opacity || "100"} onChange={e => updateSetting("window_opacity", e.target.value)} className="w-full" />
                    <div className="text-xs text-cv-subtext text-right">{settings.window_opacity || "100"}%</div>
                  </div>
                  <ToggleSetting settingKey="splash_enabled" label="Splash Screen" desc="Show animated splash on startup" />
                  <ToggleSetting settingKey="sidebar_collapsed" label="Sidebar Collapsed" desc="Start with sidebar minimized" />
                  <ToggleSetting settingKey="motion_enabled" label="Motion Effects" desc="Enable UI animations and transitions" />
                </div>
              </>
            )}

            {/* Playback */}
            {activeSection === "playback" && (
              <div className="glass-panel p-5 space-y-1">
                <h3 className="text-sm font-bold mb-3 flex items-center gap-2">
                  <Play size={16} className="text-cv-accent" /> Playback Settings
                </h3>
                <ToggleSetting settingKey="skip_intro" label="Skip Intro" desc="Auto-skip intro sequences when detected" />
                <ToggleSetting settingKey="skip_outro" label="Skip Outro / Credits" desc="Auto-skip credits and outros" />
                <ToggleSetting settingKey="auto_next" label="Auto Next Episode" desc="Automatically play next episode" />
                <ToggleSetting settingKey="auto_subtitles" label="Auto Subtitles" desc="Automatically load subtitles when available" />
                <div className="pt-3 space-y-3">
                  <div>
                    <label className="section-label">Default Player</label>
                    <select value={settings.default_player || "system"} onChange={e => updateSetting("default_player", e.target.value)} className="cv-select w-full">
                      <option value="system">System Default</option>
                      <option value="vlc">VLC Media Player</option>
                      <option value="mpv">mpv</option>
                      <option value="mpc-hc">MPC-HC</option>
                      <option value="potplayer">PotPlayer</option>
                    </select>
                  </div>
                  <div>
                    <label className="section-label">FFmpeg Path</label>
                    <input value={settings.ffmpeg_path || ""} onChange={e => updateSetting("ffmpeg_path", e.target.value)} className="cv-input" placeholder="Auto-detect or specify path" />
                  </div>
                  <div>
                    <label className="section-label">FFprobe Path</label>
                    <input value={settings.ffprobe_path || ""} onChange={e => updateSetting("ffprobe_path", e.target.value)} className="cv-input" placeholder="Auto-detect or specify path" />
                  </div>
                  <ToggleSetting settingKey="chapter_thumbs_enabled" label="Chapter Thumbnails" desc="Generate chapter preview thumbnails" />
                </div>
              </div>
            )}

            {/* Library */}
            {activeSection === "library" && (
              <div className="glass-panel p-5 space-y-1">
                <h3 className="text-sm font-bold mb-3 flex items-center gap-2">
                  <Library size={16} className="text-cv-accent" /> Library Settings
                </h3>
                <ToggleSetting settingKey="smart_collections" label="Smart Collections" desc="Auto-group media into collections" />
                <ToggleSetting settingKey="poster_sync" label="Poster Sync" desc="Sync posters from metadata providers" />
                <ToggleSetting settingKey="unified_library" label="Unified Library" desc="Show all media types in one view" />
                <ToggleSetting settingKey="watchlist_enabled" label="Watchlist" desc="Enable personal watchlist" />
                <div className="pt-3">
                  <button className="cv-btn cv-btn-danger text-xs">
                    <Trash2 size={12} /> Cleanup Missing Files
                  </button>
                </div>
              </div>
            )}

            {/* Integrations */}
            {activeSection === "integrations" && (
              <div className="glass-panel p-5">
                <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
                  <Link size={16} className="text-cv-accent" /> Integrations
                </h3>
                <div className="space-y-3">
                  {[
                    { name: "Emby SDK Compatibility", desc: "Enable Emby SDK features and API compatibility", link: "server" },
                    { name: "Plugins & Metadata", desc: "Configure metadata providers and plugin repos", link: "plugins" },
                    { name: "Live TV / IPTV", desc: "Manage Xtream Codes profiles", link: "livetv" },
                    { name: "Cloud & NAS", desc: "Connect cloud storage and NAS devices", link: "cloud" },
                    { name: "AI Diagnostics", desc: "Configure HuggingFace inference", link: "ai" },
                    { name: "Security Tools", desc: "VPN and antivirus settings", link: "security" },
                  ].map(int => (
                    <div key={int.name} className="glass-panel-2 p-3 rounded-lg flex items-center justify-between">
                      <div>
                        <div className="text-sm font-semibold">{int.name}</div>
                        <div className="text-[10px] text-cv-subtext">{int.desc}</div>
                      </div>
                      <button className="cv-btn cv-btn-secondary text-[10px] py-1 px-2">
                        Open Tab
                      </button>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Cloud Storage */}
            {activeSection === "cloud" && (
              <div className="glass-panel p-5">
                <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
                  <Cloud size={16} className="text-cv-accent" /> Cloud Storage Connections
                </h3>
                <div className="space-y-3">
                  <div className="glass-panel-2 p-4 rounded-lg space-y-3">
                    <div className="flex items-center justify-between">
                      <div>
                        <div className="text-sm font-semibold">OneDrive</div>
                        <div className="text-[10px] text-cv-subtext">{settings.cloud_onedrive_connected === "true" ? "Connected" : "Not connected"}</div>
                      </div>
                      <span className={`status-dot ${settings.cloud_onedrive_connected === "true" ? "online" : "offline"}`} />
                    </div>
                    <div className="grid grid-cols-2 gap-3">
                      <input
                        className="cv-input"
                        value={settings.cloud_onedrive_username || ""}
                        onChange={(e) => updateSetting("cloud_onedrive_username", e.target.value)}
                        placeholder="OneDrive username / email"
                      />
                      <input
                        type="password"
                        className="cv-input"
                        value={settings.cloud_onedrive_password || ""}
                        onChange={(e) => updateSetting("cloud_onedrive_password", e.target.value)}
                        placeholder="OneDrive password"
                      />
                    </div>
                    <div className="flex gap-2">
                      <button onClick={() => updateSetting("cloud_onedrive_connected", "true")} className="cv-btn cv-btn-primary text-xs"><Save size={12} /> Save + Connect</button>
                      <button onClick={() => updateSetting("cloud_onedrive_connected", "false")} className="cv-btn cv-btn-secondary text-xs">Disconnect</button>
                    </div>
                  </div>

                  <div className="glass-panel-2 p-4 rounded-lg space-y-3">
                    <div className="flex items-center justify-between">
                      <div>
                        <div className="text-sm font-semibold">Google Drive</div>
                        <div className="text-[10px] text-cv-subtext">{settings.cloud_gdrive_connected === "true" ? "Connected" : "Not connected"}</div>
                      </div>
                      <span className={`status-dot ${settings.cloud_gdrive_connected === "true" ? "online" : "offline"}`} />
                    </div>
                    <div className="grid grid-cols-2 gap-3">
                      <input
                        className="cv-input"
                        value={settings.cloud_gdrive_username || ""}
                        onChange={(e) => updateSetting("cloud_gdrive_username", e.target.value)}
                        placeholder="Google Drive username / email"
                      />
                      <input
                        type="password"
                        className="cv-input"
                        value={settings.cloud_gdrive_password || ""}
                        onChange={(e) => updateSetting("cloud_gdrive_password", e.target.value)}
                        placeholder="Google Drive password"
                      />
                    </div>
                    <div className="flex gap-2">
                      <button onClick={() => updateSetting("cloud_gdrive_connected", "true")} className="cv-btn cv-btn-primary text-xs"><Save size={12} /> Save + Connect</button>
                      <button onClick={() => updateSetting("cloud_gdrive_connected", "false")} className="cv-btn cv-btn-secondary text-xs">Disconnect</button>
                    </div>
                  </div>
                </div>
              </div>
            )}

            {/* Duplicate Finder */}
            {activeSection === "duplicates" && (
              <div className="glass-panel p-5">
                <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
                  <Copy size={16} className="text-cv-accent" /> Duplicate Finder
                </h3>
                <div className="grid grid-cols-2 gap-3 mb-4">
                  <div>
                    <label className="section-label">Match Rule</label>
                    <select value={dupMatchBy} onChange={e => setDupMatchBy(e.target.value)} className="cv-select w-full">
                      <option value="name_size">Name + Size</option>
                      <option value="name">Name Only</option>
                      <option value="size">Size Only</option>
                      <option value="hash">File Hash (slow)</option>
                    </select>
                  </div>
                  <div>
                    <label className="section-label">Size Tolerance (MB)</label>
                    <input type="number" value={dupTolerance} onChange={e => setDupTolerance(e.target.value)} className="cv-input" min="0" step="0.1" />
                  </div>
                </div>
                <div className="flex gap-2 mb-4">
                  <button onClick={runDuplicateScan} disabled={dupScanning} className="cv-btn cv-btn-primary flex-1">
                    <Search size={14} /> {dupScanning ? "Scanning..." : "Scan for Duplicates"}
                  </button>
                  <button onClick={selectAllDuplicates} disabled={dupScanning || !dupResults} className="cv-btn cv-btn-secondary flex-1">
                    <SlidersHorizontal size={14} /> Select All
                  </button>
                </div>
                <button onClick={deleteSelectedDuplicates} disabled={dupScanning || selectedDuplicates.length === 0} className="cv-btn cv-btn-danger w-full">
                  <Trash2 size={14} /> Delete Selected ({selectedDuplicates.length})
                </button>
                {dupResults && (
                  <div className="glass-panel-2 p-4 rounded-lg">
                    <div className="text-sm font-semibold mb-2">Results</div>
                <div className="space-y-4">
                  <div className="text-xs text-cv-subtext">
                    <div>Groups found: <span className="text-cv-text">{dupResults.groups_found}</span></div>
                    <div>Total duplicates: <span className="text-cv-text">{dupResults.total_duplicates}</span></div>
                    <div>Match rule: <span className="text-cv-text">{dupResults.match_rule}</span></div>
                  </div>
                  
                  {dupResults.groups && (
                    <div className="space-y-3">
                      <div className="font-semibold mb-2">Duplicate Groups:</div>
                      {dupResults.groups.map((group: any, groupIndex: number) => (
                        <div key={group.id} className="border border-white/10 rounded-lg overflow-hidden">
                          <div className="px-4 py-3 bg-cv-accent/10">
                            <div className="flex items-center justify-between">
                              <span className="font-medium">Group {groupIndex + 1}</span>
                              <span className="text-xs text-cv-subtext">{group.items.length} items</span>
                            </div>
                          </div>
                          <div className="divide-y divide-white/5">
                            {group.items.map((item: any, itemIndex: number) => {
                              const isSelected = selectedDuplicates.includes(item.id);
                              return (
                                <div 
                                  key={item.id} 
                                  className="flex items-center px-4 py-3 hover:bg-white/5 transition-colors"
                                  onClick={() => toggleDuplicateSelection(item.id)}
                                >
                                  <div className="flex-1 min-w-0">
                                    <div className="flex items-center gap-3">
                                      <div className={`h-4 w-4 rounded ${isSelected ? 'bg-cv-accent' : 'bg-white/10'} flex items-center justify-center`}>
                                        {isSelected ? <CheckCircle size={10} color={isSelected ? 'white' : 'cv-accent'} /> : <CheckCircle size={10} className="text-white/50" />}
                                      </div>
                                      <div>
                                        <div className="text-xs font-medium truncate max-w-[200px]">
                                          {item.title || 'Unknown Title'}
                                        </div>
                                        <div className="text-[9px] text-cv-subtext truncate">
                                          {item.file_path}
                                        </div>
                                        {item.file_size && (
                                          <div className="text-[9px] text-cv-subtext">
                                            {(item.file_size / (1024*1024)).toFixed(1)} MB
                                          </div>
                                        )}
                                      </div>
                                    </div>
                                  </div>
                                  <div className="text-xs text-cv-subtext">
                                    ID: {item.id}
                                  </div>
                                </div>
                              );
                            })}
                          </div>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
                  </div>
                )}
              </div>
            )}

            {/* Power & Safety */}
            {activeSection === "power" && (
              <div className="glass-panel p-5 space-y-1">
                <h3 className="text-sm font-bold mb-3 flex items-center gap-2">
                  <Zap size={16} className="text-cv-accent" /> Power & Safety
                </h3>
                <ToggleSetting settingKey="hw_transcoding" label="Hardware Transcoding" desc="Use GPU for video transcoding (requires compatible hardware)" />
                <div className="py-2.5 px-3">
                  <label className="section-label">Quality Control</label>
                  <select value={settings.quality_control || "auto"} onChange={e => updateSetting("quality_control", e.target.value)} className="cv-select w-full mt-1">
                    <option value="auto">Automatic</option>
                    <option value="original">Original Quality</option>
                    <option value="1080p">1080p Max</option>
                    <option value="720p">720p Max</option>
                    <option value="480p">480p Max</option>
                  </select>
                </div>
                <ToggleSetting settingKey="offline_mode" label="Offline Mode" desc="Disable all network features" />
                <div className="pt-3">
                  <button className="cv-btn cv-btn-secondary text-xs">
                    <Shield size={12} /> Open Security Tools
                  </button>
                </div>
              </div>
            )}
          </motion.div>
        </AnimatePresence>
      </div>
    </div>
  );
}
