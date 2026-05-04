// CinaVault Premium - Live TV Tab (Xtream Codes / Thunder TV IPTV)
import React, { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion } from "framer-motion";
import { useAppStore } from "../../store/appStore";
import { Tv, Plus, Trash2, RefreshCw, Play, Radio, Search, Calendar, Settings2 } from "lucide-react";

interface XtreamProfile {
  id?: number;
  name: string;
  server_url: string;
  username: string;
  password: string;
  output_format?: string;
  user_agent?: string;
  enabled: boolean;
  last_synced?: string | null;
}

interface LiveChannel {
  id?: number;
  profile_id: number;
  name: string;
  stream_url: string;
  logo_url?: string | null;
  group_name?: string | null;
  epg_id?: string | null;
}

export default function LiveTVTab() {
  const { addStatusMessage, setSetting } = useAppStore();
  const [profiles, setProfiles] = useState<XtreamProfile[]>([]);
  const [channels, setChannels] = useState<LiveChannel[]>([]);
  const [selectedProfile, setSelectedProfile] = useState<number | null>(null);
  const [channelSearch, setChannelSearch] = useState("");
  const [showAddProfile, setShowAddProfile] = useState(false);
  const [providerPreset, setProviderPreset] = useState<"generic" | "thunder">("generic");
  const [newProfile, setNewProfile] = useState({
    name: "",
    server_url: "",
    username: "",
    password: "",
    output_format: "ts",
    user_agent: "",
  });

  useEffect(() => {
    loadProfiles();
  }, []);

  const persistIptvSetting = async (key: string, value: string) => {
    setSetting(key, value);
    try {
      await invoke("set_setting", { key, value });
    } catch {
      // ignore in dev mode
    }
  };

  const applyPreset = (preset: "generic" | "thunder") => {
    setProviderPreset(preset);
    if (preset === "thunder") {
      setNewProfile((prev) => ({
        ...prev,
        name: prev.name || "Thunder TV IPTV",
        output_format: "m3u8",
        user_agent: prev.user_agent || "VLC/3.0.20 LibVLC/3.0.20",
      }));
      persistIptvSetting("iptv_preferred_engine", "thunder_tv");
      persistIptvSetting("iptv_preferred_output", "m3u8");
    } else {
      setNewProfile((prev) => ({ ...prev, output_format: "ts" }));
      persistIptvSetting("iptv_preferred_engine", "xtream");
      persistIptvSetting("iptv_preferred_output", "ts");
    }
  };

  const loadProfiles = async () => {
    try {
      const loaded = await invoke<XtreamProfile[]>("get_xtream_profiles");
      setProfiles(loaded);
    } catch {
      setProfiles(DEMO_PROFILES);
    }
  };

  const addProfile = async () => {
    if (!newProfile.name || !newProfile.server_url || !newProfile.username || !newProfile.password) return;
    try {
      await invoke("add_xtream_profile", {
        name: newProfile.name,
        server_url: newProfile.server_url,
        username: newProfile.username,
        password: newProfile.password,
        output_format: newProfile.output_format,
        user_agent: newProfile.user_agent,
      });
      addStatusMessage(`IPTV profile added: ${newProfile.name}`);
      setNewProfile({ name: "", server_url: "", username: "", password: "", output_format: "ts", user_agent: "" });
      setShowAddProfile(false);
      await loadProfiles();
    } catch (e) {
      addStatusMessage(`Failed: ${e}`);
    }
  };

  const removeProfile = async (id: number) => {
    try {
      await invoke("remove_xtream_profile", { id });
      addStatusMessage("Profile removed");
      await loadProfiles();
      if (selectedProfile === id) {
        setSelectedProfile(null);
        setChannels([]);
      }
    } catch (e) {
      addStatusMessage(`Failed: ${e}`);
    }
  };

  const syncStreams = async (id: number) => {
    addStatusMessage("Syncing Xtream streams...");
    try {
      const result = await invoke<any>("sync_xtream_streams", { profileId: id });
      addStatusMessage(`Synced ${result.channels_synced} channels`);
      await loadChannels(id);
    } catch (e) {
      addStatusMessage(`Sync failed: ${e}`);
    }
  };

  const syncEpg = async (id: number) => {
    addStatusMessage("Syncing EPG data...");
    try {
      await invoke("sync_epg", { profileId: id });
      addStatusMessage("EPG synced successfully");
    } catch (e) {
      addStatusMessage(`EPG sync failed: ${e}`);
    }
  };

  const loadChannels = async (profileId: number) => {
    setSelectedProfile(profileId);
    try {
      const loaded = await invoke<LiveChannel[]>("get_live_channels", { profileId });
      setChannels(loaded);
    } catch {
      setChannels(DEMO_CHANNELS);
    }
  };

  const playChannel = async (streamUrl: string) => {
    try {
      await invoke("play_channel", { streamUrl });
      addStatusMessage("Starting IPTV playback...");
    } catch (e) {
      addStatusMessage(`Playback failed: ${e}`);
    }
  };

  const filteredChannels = useMemo(
    () => channels.filter((ch) => !channelSearch || ch.name.toLowerCase().includes(channelSearch.toLowerCase())),
    [channels, channelSearch],
  );

  const groupedChannels = useMemo(() => filteredChannels.reduce((acc: Record<string, LiveChannel[]>, ch) => {
    const group = ch.group_name || "Uncategorized";
    if (!acc[group]) acc[group] = [];
    acc[group].push(ch);
    return acc;
  }, {}), [filteredChannels]);

  return (
    <div className="space-y-5">
      <div className="glass-panel p-5">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-sm font-bold flex items-center gap-2">
            <Radio size={16} className="text-cv-accent" /> Xtream / Thunder TV IPTV Profiles
          </h3>
          <button onClick={() => setShowAddProfile(!showAddProfile)} className="cv-btn cv-btn-primary text-xs">
            <Plus size={12} /> Add Profile
          </button>
        </div>

        {showAddProfile && (
          <motion.div initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} className="mb-4 glass-panel-2 p-4 rounded-lg space-y-3">
            <div className="flex items-center gap-2">
              <button onClick={() => applyPreset("generic")} className={`cv-btn ${providerPreset === "generic" ? "cv-btn-primary" : "cv-btn-secondary"} text-xs`}>
                Generic Xtream
              </button>
              <button onClick={() => applyPreset("thunder")} className={`cv-btn ${providerPreset === "thunder" ? "cv-btn-primary" : "cv-btn-secondary"} text-xs`}>
                Thunder TV IPTV
              </button>
            </div>

            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="section-label">Profile Name</label>
                <input value={newProfile.name} onChange={(e) => setNewProfile({ ...newProfile, name: e.target.value })} className="cv-input" placeholder="My IPTV" />
              </div>
              <div>
                <label className="section-label">Server URL</label>
                <input value={newProfile.server_url} onChange={(e) => setNewProfile({ ...newProfile, server_url: e.target.value })} className="cv-input" placeholder="http://provider.com:8080" />
              </div>
              <div>
                <label className="section-label">Username</label>
                <input value={newProfile.username} onChange={(e) => setNewProfile({ ...newProfile, username: e.target.value })} className="cv-input" />
              </div>
              <div>
                <label className="section-label">Password</label>
                <input type="password" value={newProfile.password} onChange={(e) => setNewProfile({ ...newProfile, password: e.target.value })} className="cv-input" />
              </div>
              <div>
                <label className="section-label">Output Format</label>
                <select value={newProfile.output_format} onChange={(e) => setNewProfile({ ...newProfile, output_format: e.target.value })} className="cv-select w-full">
                  <option value="ts">MPEG-TS (.ts)</option>
                  <option value="m3u8">HLS (.m3u8)</option>
                </select>
              </div>
              <div>
                <label className="section-label">User Agent (optional)</label>
                <input value={newProfile.user_agent} onChange={(e) => setNewProfile({ ...newProfile, user_agent: e.target.value })} className="cv-input" placeholder="Use provider-recommended agent" />
              </div>
            </div>

            <div className="flex gap-2 mt-3">
              <button onClick={addProfile} className="cv-btn cv-btn-primary text-xs"><Plus size={12} /> Save</button>
              <button onClick={() => setShowAddProfile(false)} className="cv-btn cv-btn-secondary text-xs">Cancel</button>
            </div>
          </motion.div>
        )}

        <div className="text-xs text-cv-subtext mb-3 flex items-center gap-2">
          <Settings2 size={12} /> Robust playback prioritizes VLC/mpv compatibility for Xtream and Thunder TV streams.
        </div>

        {profiles.length === 0 ? (
          <div className="text-center py-6 text-cv-subtext text-sm">No IPTV profiles configured</div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
            {profiles.map((profile) => (
              <div
                key={profile.id}
                className={`glass-panel-2 p-4 rounded-lg cursor-pointer transition-all ${selectedProfile === profile.id ? "ring-1 ring-cv-accent" : "hover:bg-white/5"}`}
                onClick={() => profile.id && loadChannels(profile.id)}
              >
                <div className="flex items-center justify-between mb-2">
                  <span className="text-sm font-semibold">{profile.name}</span>
                  <span className={`status-dot ${profile.enabled ? "online" : "offline"}`} />
                </div>
                <div className="text-[10px] text-cv-subtext mb-1 truncate">{profile.server_url}</div>
                <div className="text-[10px] text-cv-subtext mb-3">Format: {(profile.output_format || "ts").toUpperCase()}</div>
                <div className="flex gap-1">
                  <button onClick={(e) => { e.stopPropagation(); profile.id && syncStreams(profile.id); }} className="cv-btn cv-btn-secondary text-[10px] py-1 px-2"><RefreshCw size={10} /> Sync</button>
                  <button onClick={(e) => { e.stopPropagation(); profile.id && syncEpg(profile.id); }} className="cv-btn cv-btn-secondary text-[10px] py-1 px-2"><Calendar size={10} /> EPG</button>
                  <button onClick={(e) => { e.stopPropagation(); profile.id && removeProfile(profile.id); }} className="cv-btn cv-btn-danger text-[10px] py-1 px-2"><Trash2 size={10} /></button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {selectedProfile && (
        <div className="glass-panel rounded-xl overflow-hidden">
          <div className="px-5 py-3 border-b border-white/5 flex items-center justify-between">
            <h3 className="text-sm font-bold">{channels.length} Channels</h3>
            <div className="relative w-56">
              <Search size={12} className="absolute left-3 top-1/2 -translate-y-1/2 text-cv-subtext" />
              <input value={channelSearch} onChange={(e) => setChannelSearch(e.target.value)} className="cv-input pl-8 text-xs py-1.5" placeholder="Search channels..." />
            </div>
          </div>
          <div className="max-h-[calc(100vh-420px)] overflow-y-auto">
            {Object.entries(groupedChannels).map(([group, chs]) => (
              <div key={group}>
                <div className="px-5 py-2 bg-white/[0.02] text-[10px] font-bold uppercase tracking-wider text-cv-accent">{group} ({chs.length})</div>
                {chs.map((channel, i) => (
                  <div key={channel.id || i} className="px-5 py-2 flex items-center gap-3 zebra-row cursor-pointer group" onClick={() => playChannel(channel.stream_url)}>
                    <div className="w-8 h-8 rounded bg-white/5 flex items-center justify-center shrink-0">
                      {channel.logo_url ? <img src={channel.logo_url} className="w-6 h-6 object-contain" /> : <Tv size={14} className="text-cv-subtext" />}
                    </div>
                    <span className="text-sm flex-1 truncate">{channel.name}</span>
                    <button className="opacity-0 group-hover:opacity-100 cv-btn cv-btn-primary text-[10px] py-1 px-2">
                      <Play size={10} /> Play
                    </button>
                  </div>
                ))}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

const DEMO_PROFILES: XtreamProfile[] = [
  {
    id: 1,
    name: "Thunder TV IPTV",
    server_url: "http://iptv.example.com:8080",
    username: "user",
    password: "pass",
    output_format: "m3u8",
    user_agent: "VLC/3.0.20 LibVLC/3.0.20",
    enabled: true,
    last_synced: null,
  },
];

const DEMO_CHANNELS: LiveChannel[] = [
  { id: 1, profile_id: 1, name: "CNN", stream_url: "", logo_url: null, group_name: "News", epg_id: "cnn" },
  { id: 2, profile_id: 1, name: "ESPN", stream_url: "", logo_url: null, group_name: "Sports", epg_id: "espn" },
  { id: 3, profile_id: 1, name: "HBO", stream_url: "", logo_url: null, group_name: "Entertainment", epg_id: "hbo" },
];
