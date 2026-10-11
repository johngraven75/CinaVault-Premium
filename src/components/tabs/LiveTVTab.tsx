// CinaVault Premium — Live TV Tab (Xtream Codes / M3U playlists / EPG)
import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion } from "framer-motion";
import { useAppStore } from "../../store/appStore";
import {
  ArrowLeft,
  Calendar,
  ChevronDown,
  ChevronUp,
  Clock,
  Link2,
  ListVideo,
  Play,
  Plus,
  Radio,
  RefreshCw,
  Search,
  Trash2,
  Tv,
} from "lucide-react";
import { buildAddXtreamProfileArgs } from "../../utils/xtreamProfile";
import {
  buildAddM3uArgs,
  dayLabel,
  formatGuideTime,
  isPlayableUrl,
  programmeProgress,
  remainingLabel,
  splitGuide,
  type GuideProgramme,
} from "../../utils/iptvStream";
import IPTVPlayer from "../IPTVPlayer";
import TabBanner from "../experience/TabBanner";

interface LiveProfile {
  id: number;
  name: string;
  server_url: string;
  username: string;
  password: string;
  enabled: boolean;
  last_synced: string | null;
  source_kind?: "xtream" | "m3u";
}

interface LiveChannel {
  id: number;
  profile_id: number;
  name: string;
  stream_url: string;
  logo_url: string | null;
  group_name: string | null;
  epg_id: string | null;
}

interface EpgSource {
  id: number;
  name: string;
  url: string;
  profile_id: number | null;
  enabled: boolean;
  last_synced: string | null;
  last_error: string | null;
  channel_count: number;
  programme_count: number;
}

type SourceType = "xtream" | "m3u";

const nowSeconds = () => Math.floor(Date.now() / 1000);

/** Re-renders every 30 s so progress bars and "on now" stay current. */
function useClock(): number {
  const [now, setNow] = useState(nowSeconds);
  useEffect(() => {
    const timer = window.setInterval(() => setNow(nowSeconds()), 30_000);
    return () => window.clearInterval(timer);
  }, []);
  return now;
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export default function LiveTVTab() {
  const { addStatusMessage } = useAppStore();
  const now = useClock();
  const [profiles, setProfiles] = useState<LiveProfile[]>([]);
  const [channels, setChannels] = useState<LiveChannel[]>([]);
  const [selectedProfile, setSelectedProfile] = useState<number | null>(null);
  const [channelSearch, setChannelSearch] = useState("");
  const [showAddProfile, setShowAddProfile] = useState(false);
  const [sourceType, setSourceType] = useState<SourceType>("xtream");
  const [newProfile, setNewProfile] = useState({ name: "", server_url: "", username: "", password: "" });
  const [newPlaylist, setNewPlaylist] = useState({ name: "", url: "" });
  const [selectedChannel, setSelectedChannel] = useState<LiveChannel | null>(null);
  const [directUrl, setDirectUrl] = useState("");
  const [busy, setBusy] = useState<string | null>(null);

  const [epgSources, setEpgSources] = useState<EpgSource[]>([]);
  const [newEpg, setNewEpg] = useState({ name: "", url: "" });
  const [showGuideSources, setShowGuideSources] = useState(false);
  const [guide, setGuide] = useState<GuideProgramme[]>([]);
  const [guideLoading, setGuideLoading] = useState(false);
  const [nowPlaying, setNowPlaying] = useState<Record<string, GuideProgramme>>({});
  const [guideVersion, setGuideVersion] = useState(0);
  const [expandedProgramme, setExpandedProgramme] = useState<number | null>(null);

  const loadProfiles = useCallback(async () => {
    try {
      setProfiles(await invoke<LiveProfile[]>("get_xtream_profiles"));
    } catch {
      setProfiles(DEMO_PROFILES);
    }
  }, []);

  const loadEpgSources = useCallback(async () => {
    try {
      setEpgSources(await invoke<EpgSource[]>("list_epg_sources"));
    } catch {
      setEpgSources([]);
    }
  }, []);

  useEffect(() => {
    void loadProfiles();
    void loadEpgSources();
    // Refresh guides older than 12 hours in the background.
    invoke<{ synced: number }>("sync_stale_epg_sources")
      .then((result) => {
        if (result.synced > 0) {
          setGuideVersion((value) => value + 1);
          void loadEpgSources();
        }
      })
      .catch(() => undefined);
  }, [loadProfiles, loadEpgSources]);

  // ── Sources ────────────────────────────────────────────────────────────────

  const addProfile = async () => {
    try {
      if (sourceType === "m3u") {
        const args = buildAddM3uArgs(newPlaylist);
        const id = await invoke<number>("add_m3u_playlist", args);
        addStatusMessage(`Playlist added: ${args.name}`);
        setNewPlaylist({ name: "", url: "" });
        setShowAddProfile(false);
        await loadProfiles();
        await syncStreams(id);
      } else {
        const profileArgs = buildAddXtreamProfileArgs(newProfile);
        await invoke("add_xtream_profile", profileArgs);
        addStatusMessage(`IPTV profile added: ${profileArgs.name}`);
        setNewProfile({ name: "", server_url: "", username: "", password: "" });
        setShowAddProfile(false);
        await loadProfiles();
      }
    } catch (e) {
      addStatusMessage(`Failed: ${errorText(e)}`);
    }
  };

  const removeProfile = async (id: number) => {
    try {
      await invoke("remove_xtream_profile", { id });
      addStatusMessage("Profile removed");
      await loadProfiles();
      await loadEpgSources();
      if (selectedProfile === id) {
        setSelectedProfile(null);
        setChannels([]);
        setSelectedChannel(null);
      }
    } catch (e) {
      addStatusMessage(`Failed: ${errorText(e)}`);
    }
  };

  const syncStreams = async (id: number) => {
    setBusy(`streams-${id}`);
    addStatusMessage("Syncing channels...");
    try {
      const result = await invoke<{ channels_synced: number; guides_found?: number }>("sync_xtream_streams", { profileId: id });
      const guides = result.guides_found ? `, found ${result.guides_found} guide source${result.guides_found === 1 ? "" : "s"}` : "";
      addStatusMessage(`Synced ${result.channels_synced} channels${guides}`);
      await loadProfiles();
      await loadChannels(id);
      if (result.guides_found) {
        await loadEpgSources();
        await syncEpg(id);
      }
    } catch (e) {
      addStatusMessage(`Sync failed: ${errorText(e)}`);
    } finally {
      setBusy(null);
    }
  };

  const syncEpg = async (id: number) => {
    setBusy(`epg-${id}`);
    addStatusMessage("Syncing programme guide...");
    try {
      const result = await invoke<{ programmes?: number }>("sync_epg", { profileId: id });
      addStatusMessage(`Guide synced: ${result.programmes ?? 0} programmes`);
      setGuideVersion((value) => value + 1);
    } catch (e) {
      addStatusMessage(`EPG sync failed: ${errorText(e)}`);
    } finally {
      setBusy(null);
      await loadEpgSources();
    }
  };

  const addEpgSource = async () => {
    const url = newEpg.url.trim();
    if (!isPlayableUrl(url)) {
      addStatusMessage("Enter a full guide URL starting with http:// or https://");
      return;
    }
    try {
      const id = await invoke<number>("add_epg_source", { name: newEpg.name.trim(), url });
      setNewEpg({ name: "", url: "" });
      await loadEpgSources();
      await syncEpgSource(id);
    } catch (e) {
      addStatusMessage(`Failed: ${errorText(e)}`);
    }
  };

  const syncEpgSource = async (id: number) => {
    setBusy(`source-${id}`);
    try {
      const result = await invoke<{ programmes: number; channels: number }>("sync_epg_source", { id });
      addStatusMessage(`Guide synced: ${result.programmes} programmes on ${result.channels} channels`);
      setGuideVersion((value) => value + 1);
    } catch (e) {
      addStatusMessage(`Guide sync failed: ${errorText(e)}`);
    } finally {
      setBusy(null);
      await loadEpgSources();
    }
  };

  const removeEpgSource = async (id: number) => {
    try {
      await invoke("remove_epg_source", { id });
      await loadEpgSources();
      setGuideVersion((value) => value + 1);
    } catch (e) {
      addStatusMessage(`Failed: ${errorText(e)}`);
    }
  };

  // ── Channels and guide ─────────────────────────────────────────────────────

  const loadChannels = async (profileId: number) => {
    setSelectedProfile(profileId);
    setSelectedChannel(null);
    try {
      setChannels(await invoke<LiveChannel[]>("get_live_channels", { profileId }));
    } catch {
      setChannels(DEMO_CHANNELS);
    }
  };

  // "On now" for the channel list, refreshed with the clock.
  const nowBucket = Math.floor(now / 300);
  useEffect(() => {
    if (!channels.length) {
      setNowPlaying({});
      return;
    }
    let cancelled = false;
    invoke<Record<string, GuideProgramme>>("get_now_playing", {
      channels: channels.map((ch) => ({ key: String(ch.id), epg_id: ch.epg_id, name: ch.name })),
    })
      .then((map) => !cancelled && setNowPlaying(map))
      .catch(() => !cancelled && setNowPlaying({}));
    return () => {
      cancelled = true;
    };
  }, [channels, guideVersion, nowBucket]);

  // Full guide for the channel being watched.
  useEffect(() => {
    if (!selectedChannel) {
      setGuide([]);
      return;
    }
    let cancelled = false;
    setGuideLoading(true);
    invoke<GuideProgramme[]>("get_channel_guide", {
      epgId: selectedChannel.epg_id,
      name: selectedChannel.name,
      limit: 40,
    })
      .then((items) => !cancelled && setGuide(items))
      .catch(() => !cancelled && setGuide([]))
      .finally(() => !cancelled && setGuideLoading(false));
    return () => {
      cancelled = true;
    };
  }, [selectedChannel, guideVersion, nowBucket]);

  const filteredChannels = useMemo(() => {
    const query = channelSearch.trim().toLowerCase();
    if (!query) return channels;
    return channels.filter(
      (ch) => ch.name.toLowerCase().includes(query) || nowPlaying[String(ch.id)]?.title.toLowerCase().includes(query),
    );
  }, [channels, channelSearch, nowPlaying]);

  const groupedChannels = useMemo(
    () =>
      filteredChannels.reduce((acc: Record<string, LiveChannel[]>, ch) => {
        const group = ch.group_name || "Uncategorized";
        (acc[group] ||= []).push(ch);
        return acc;
      }, {}),
    [filteredChannels],
  );

  const zap = (step: number) => {
    if (!selectedChannel || !filteredChannels.length) return;
    const index = filteredChannels.findIndex((ch) => ch.id === selectedChannel.id);
    const next = filteredChannels[(index + step + filteredChannels.length) % filteredChannels.length];
    setExpandedProgramme(null);
    setSelectedChannel(next);
  };

  const playDirect = () => {
    const url = directUrl.trim();
    if (!isPlayableUrl(url)) {
      addStatusMessage("Enter a stream URL starting with http:// or https://");
      return;
    }
    setSelectedChannel({ id: -1, profile_id: -1, name: "Direct stream", stream_url: url, logo_url: null, group_name: null, epg_id: null });
  };

  const { current, upcoming } = splitGuide(guide, now);
  const watching = selectedChannel;
  const playerOpen = !!watching;

  return (
    <div className="space-y-5">
      <TabBanner icon={Tv} eyebrow="Broadcast Fabric" title="Live Signal" subtitle="Navigate channels, guide intelligence, and live streams through a unified cinematic interface." accent="from-fuchsia-400/32 to-purple-500/10" accentText="text-fuchsia-100" />

      {/* Player + guide */}
      {playerOpen && watching && (
        <div className="grid grid-cols-1 gap-4 xl:grid-cols-[minmax(0,2fr)_minmax(300px,1fr)]">
          <div className="glass-panel overflow-hidden rounded-xl">
            <div className="flex items-center justify-between gap-3 border-b border-white/5 px-4 py-2.5">
              <button onClick={() => setSelectedChannel(null)} className="flex items-center gap-1.5 text-xs text-cv-accent hover:text-cv-accent/80">
                <ArrowLeft size={14} /> Channels
              </button>
              <div className="flex min-w-0 items-center gap-2">
                {watching.logo_url ? (
                  <img src={watching.logo_url} className="h-6 w-6 shrink-0 object-contain" alt="" />
                ) : (
                  <Tv size={14} className="shrink-0 text-cv-subtext" />
                )}
                <h3 className="truncate text-sm font-bold">{watching.name}</h3>
              </div>
              <div className="flex items-center gap-1">
                <button onClick={() => zap(-1)} disabled={watching.id < 0} className="cv-btn cv-btn-secondary px-2 py-1 text-[10px]" title="Previous channel">
                  <ChevronUp size={12} />
                </button>
                <button onClick={() => zap(1)} disabled={watching.id < 0} className="cv-btn cv-btn-secondary px-2 py-1 text-[10px]" title="Next channel">
                  <ChevronDown size={12} />
                </button>
              </div>
            </div>
            <div className="p-3">
              <IPTVPlayer
                streamUrl={watching.stream_url}
                title={watching.name}
                subtitle={current ? `${current.title}${current.episode ? ` · ${current.episode}` : ""}` : null}
              />
              {current && (
                <div className="mt-3 rounded-lg bg-white/[0.03] p-3">
                  <div className="flex items-center justify-between gap-3 text-[10px] uppercase tracking-wider text-cv-subtext">
                    <span className="font-bold text-cv-accent">On now</span>
                    <span>
                      {formatGuideTime(current.start)} – {formatGuideTime(current.stop)} · {remainingLabel(current, now)}
                    </span>
                  </div>
                  <div className="mt-1 text-sm font-semibold">{current.title}</div>
                  {(current.sub_title || current.episode) && (
                    <div className="text-xs text-cv-subtext">{[current.sub_title, current.episode].filter(Boolean).join(" · ")}</div>
                  )}
                  <div className="mt-2 h-1 overflow-hidden rounded-full bg-white/10">
                    <div className="h-full rounded-full bg-cv-accent" style={{ width: `${programmeProgress(current, now) * 100}%` }} />
                  </div>
                  {current.description && <p className="mt-2 line-clamp-3 text-xs leading-relaxed text-cv-subtext">{current.description}</p>}
                </div>
              )}
            </div>
          </div>

          <div className="glass-panel flex max-h-[calc(100vh-220px)] min-h-[320px] flex-col overflow-hidden rounded-xl">
            <div className="flex items-center justify-between border-b border-white/5 px-4 py-2.5">
              <h3 className="flex items-center gap-2 text-sm font-bold">
                <Calendar size={14} className="text-cv-accent" /> Programme guide
              </h3>
              {guideLoading && <RefreshCw size={12} className="animate-spin text-cv-subtext" />}
            </div>
            <div className="flex-1 overflow-y-auto">
              {!guideLoading && !current && upcoming.length === 0 ? (
                <div className="px-5 py-8 text-center text-xs leading-relaxed text-cv-subtext">
                  {epgSources.length === 0
                    ? "No guide data yet. Add an EPG source below, or sync the guide from your provider profile."
                    : "No guide listings match this channel. The guide may use a different channel name or ID."}
                </div>
              ) : (
                <ol>
                  {[...(current ? [current] : []), ...upcoming].map((item) => {
                    const isNow = item === current;
                    const day = dayLabel(item.start, now);
                    const open = expandedProgramme === item.start;
                    return (
                      <li key={`${item.start}-${item.title}`}>
                        <button
                          onClick={() => setExpandedProgramme(open ? null : item.start)}
                          className={`flex w-full gap-3 px-4 py-2.5 text-left transition-colors hover:bg-white/5 ${isNow ? "bg-cv-accent/10" : ""}`}
                        >
                          <div className="w-14 shrink-0 pt-0.5 text-right">
                            <div className={`text-xs font-semibold tabular-nums ${isNow ? "text-cv-accent" : ""}`}>{formatGuideTime(item.start)}</div>
                            {day && <div className="text-[10px] text-cv-subtext">{day}</div>}
                          </div>
                          <div className="min-w-0 flex-1">
                            <div className="flex items-center gap-2">
                              <span className={`truncate text-sm ${isNow ? "font-bold" : "font-medium"}`}>{item.title}</span>
                              {isNow && <span className="shrink-0 rounded bg-red-600/80 px-1.5 text-[9px] font-bold uppercase text-white">Now</span>}
                            </div>
                            <div className="truncate text-[11px] text-cv-subtext">
                              {[item.episode, item.category, `until ${formatGuideTime(item.stop)}`].filter(Boolean).join(" · ")}
                            </div>
                            {isNow && (
                              <div className="mt-1.5 h-0.5 overflow-hidden rounded-full bg-white/10">
                                <div className="h-full bg-cv-accent" style={{ width: `${programmeProgress(item, now) * 100}%` }} />
                              </div>
                            )}
                            {open && (item.sub_title || item.description) && (
                              <div className="mt-2 space-y-1 text-xs leading-relaxed text-cv-subtext">
                                {item.sub_title && <div className="font-medium text-white/80">{item.sub_title}</div>}
                                {item.description && <p>{item.description}</p>}
                              </div>
                            )}
                          </div>
                        </button>
                      </li>
                    );
                  })}
                </ol>
              )}
            </div>
          </div>
        </div>
      )}

      {/* Sources */}
      <div className="glass-panel p-5">
        <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
          <h3 className="flex items-center gap-2 text-sm font-bold">
            <Radio size={16} className="text-cv-accent" /> IPTV Sources
          </h3>
          <div className="flex items-center gap-2">
            <div className="relative">
              <Link2 size={12} className="absolute left-3 top-1/2 -translate-y-1/2 text-cv-subtext" />
              <input
                value={directUrl}
                onChange={(e) => setDirectUrl(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && playDirect()}
                className="cv-input w-64 py-1.5 pl-8 text-xs"
                placeholder="Play a stream URL (.m3u8, .ts)…"
              />
            </div>
            <button onClick={playDirect} className="cv-btn cv-btn-secondary text-xs">
              <Play size={12} /> Play
            </button>
            <button onClick={() => setShowAddProfile(!showAddProfile)} className="cv-btn cv-btn-primary text-xs">
              <Plus size={12} /> Add Source
            </button>
          </div>
        </div>

        {showAddProfile && (
          <motion.div initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} className="glass-panel-2 mb-4 rounded-lg p-4">
            <div className="mb-3 flex gap-1">
              {(
                [
                  ["xtream", "Xtream Codes login"],
                  ["m3u", "M3U playlist URL"],
                ] as const
              ).map(([value, label]) => (
                <button
                  key={value}
                  onClick={() => setSourceType(value)}
                  className={`cv-btn text-xs ${sourceType === value ? "cv-btn-primary" : "cv-btn-secondary"}`}
                >
                  {label}
                </button>
              ))}
            </div>
            {sourceType === "xtream" ? (
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
              </div>
            ) : (
              <div className="grid grid-cols-[1fr_2fr] gap-3">
                <div>
                  <label className="section-label">Name</label>
                  <input value={newPlaylist.name} onChange={(e) => setNewPlaylist({ ...newPlaylist, name: e.target.value })} className="cv-input" placeholder="My playlist" />
                </div>
                <div>
                  <label className="section-label">Playlist URL (M3U / M3U8)</label>
                  <input value={newPlaylist.url} onChange={(e) => setNewPlaylist({ ...newPlaylist, url: e.target.value })} className="cv-input" placeholder="http://provider.com/get.php?username=…&type=m3u_plus" />
                </div>
              </div>
            )}
            <div className="mt-3 flex gap-2">
              <button onClick={addProfile} className="cv-btn cv-btn-primary text-xs">
                <Plus size={12} /> Save
              </button>
              <button onClick={() => setShowAddProfile(false)} className="cv-btn cv-btn-secondary text-xs">
                Cancel
              </button>
            </div>
          </motion.div>
        )}

        {profiles.length === 0 ? (
          <div className="py-6 text-center text-sm text-cv-subtext">No IPTV sources yet. Add an Xtream Codes login or an M3U playlist.</div>
        ) : (
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2 lg:grid-cols-3">
            {profiles.map((p) => (
              <div
                key={p.id}
                className={`glass-panel-2 cursor-pointer rounded-lg p-4 transition-all ${selectedProfile === p.id ? "ring-1 ring-cv-accent" : "hover:bg-white/5"}`}
                onClick={() => loadChannels(p.id)}
              >
                <div className="mb-2 flex items-center justify-between">
                  <span className="flex items-center gap-2 text-sm font-semibold">
                    {p.source_kind === "m3u" ? <ListVideo size={13} className="text-cv-accent" /> : <Radio size={13} className="text-cv-accent" />}
                    {p.name}
                  </span>
                  <span className={`status-dot ${p.enabled ? "online" : "offline"}`} />
                </div>
                <div className="mb-3 truncate text-[10px] text-cv-subtext">
                  {p.source_kind === "m3u" ? "M3U playlist" : p.server_url}
                  {p.last_synced ? ` · synced ${new Date(p.last_synced).toLocaleString()}` : " · not synced yet"}
                </div>
                <div className="flex gap-1">
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      void syncStreams(p.id);
                    }}
                    disabled={busy !== null}
                    className="cv-btn cv-btn-secondary px-2 py-1 text-[10px]"
                  >
                    <RefreshCw size={10} className={busy === `streams-${p.id}` ? "animate-spin" : ""} /> Sync
                  </button>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      void syncEpg(p.id);
                    }}
                    disabled={busy !== null}
                    className="cv-btn cv-btn-secondary px-2 py-1 text-[10px]"
                  >
                    <Calendar size={10} className={busy === `epg-${p.id}` ? "animate-spin" : ""} /> EPG
                  </button>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      void removeProfile(p.id);
                    }}
                    className="cv-btn cv-btn-danger px-2 py-1 text-[10px]"
                  >
                    <Trash2 size={10} />
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}

        {/* EPG sources */}
        <div className="mt-5 border-t border-white/5 pt-4">
          <button onClick={() => setShowGuideSources(!showGuideSources)} className="flex w-full items-center justify-between text-left">
            <span className="flex items-center gap-2 text-xs font-bold uppercase tracking-wider text-cv-subtext">
              <Calendar size={12} className="text-cv-accent" /> Programme guide sources (XMLTV)
              <span className="rounded bg-white/5 px-1.5 py-0.5 text-[10px] normal-case tracking-normal">{epgSources.length}</span>
            </span>
            {showGuideSources ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
          </button>
          {showGuideSources && (
            <div className="mt-3 space-y-3">
              <div className="flex flex-wrap gap-2">
                <input value={newEpg.name} onChange={(e) => setNewEpg({ ...newEpg, name: e.target.value })} className="cv-input w-40 text-xs" placeholder="Name (optional)" />
                <input
                  value={newEpg.url}
                  onChange={(e) => setNewEpg({ ...newEpg, url: e.target.value })}
                  onKeyDown={(e) => e.key === "Enter" && addEpgSource()}
                  className="cv-input min-w-[260px] flex-1 text-xs"
                  placeholder="https://example.com/guide.xml or guide.xml.gz"
                />
                <button onClick={addEpgSource} disabled={busy !== null} className="cv-btn cv-btn-primary text-xs">
                  <Plus size={12} /> Add guide
                </button>
              </div>
              {epgSources.length === 0 ? (
                <p className="text-xs text-cv-subtext">
                  Add an XMLTV guide URL to show what is on. M3U playlists that name a guide and Xtream logins (EPG button) add theirs automatically.
                </p>
              ) : (
                <div className="space-y-2">
                  {epgSources.map((source) => (
                    <div key={source.id} className="glass-panel-2 flex items-center gap-3 rounded-lg px-3 py-2">
                      <div className="min-w-0 flex-1">
                        <div className="truncate text-xs font-semibold">{source.name}</div>
                        <div className="truncate text-[10px] text-cv-subtext">{source.url}</div>
                        <div className={`text-[10px] ${source.last_error ? "text-amber-400" : "text-cv-subtext"}`}>
                          {source.last_error
                            ? `Last sync failed: ${source.last_error}`
                            : source.last_synced
                              ? `${source.programme_count.toLocaleString()} programmes · ${source.channel_count.toLocaleString()} channels · synced ${new Date(source.last_synced).toLocaleString()}`
                              : "Not synced yet"}
                        </div>
                      </div>
                      <button onClick={() => syncEpgSource(source.id)} disabled={busy !== null} className="cv-btn cv-btn-secondary px-2 py-1 text-[10px]" title="Sync now">
                        <RefreshCw size={10} className={busy === `source-${source.id}` ? "animate-spin" : ""} />
                      </button>
                      <button onClick={() => removeEpgSource(source.id)} className="cv-btn cv-btn-danger px-2 py-1 text-[10px]" title="Remove">
                        <Trash2 size={10} />
                      </button>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>
      </div>

      {/* Channel list */}
      {selectedProfile && (
        <div className="glass-panel overflow-hidden rounded-xl">
          <div className="flex items-center justify-between border-b border-white/5 px-5 py-3">
            <h3 className="text-sm font-bold">{channels.length.toLocaleString()} Channels</h3>
            <div className="relative w-56">
              <Search size={12} className="absolute left-3 top-1/2 -translate-y-1/2 text-cv-subtext" />
              <input value={channelSearch} onChange={(e) => setChannelSearch(e.target.value)} className="cv-input py-1.5 pl-8 text-xs" placeholder="Search channels or shows..." />
            </div>
          </div>
          <div className="max-h-[calc(100vh-420px)] overflow-y-auto">
            {Object.entries(groupedChannels).map(([group, chs]) => (
              <div key={group}>
                <div className="sticky top-0 z-10 bg-[rgba(4,10,20,0.92)] px-5 py-2 text-[10px] font-bold uppercase tracking-wider text-cv-accent backdrop-blur">
                  {group} ({chs.length})
                </div>
                {chs.map((ch) => {
                  const onNow = nowPlaying[String(ch.id)];
                  const active = watching?.id === ch.id;
                  return (
                    <div
                      key={ch.id}
                      className={`group zebra-row flex cursor-pointer items-center gap-3 px-5 py-2 ${active ? "bg-cv-accent/10" : ""}`}
                      onClick={() => {
                        setExpandedProgramme(null);
                        setSelectedChannel(ch);
                        window.scrollTo({ top: 0, behavior: "smooth" });
                      }}
                    >
                      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded bg-white/5">
                        {ch.logo_url ? <img src={ch.logo_url} className="h-6 w-6 object-contain" alt="" loading="lazy" /> : <Tv size={14} className="text-cv-subtext" />}
                      </div>
                      <div className="min-w-0 flex-1">
                        <div className="truncate text-sm">{ch.name}</div>
                        {onNow && (
                          <div className="flex items-center gap-2">
                            <span className="truncate text-[11px] text-cv-subtext">
                              <Clock size={9} className="mr-1 inline" />
                              {onNow.title}
                            </span>
                            <div className="h-0.5 w-12 shrink-0 overflow-hidden rounded-full bg-white/10">
                              <div className="h-full bg-cv-accent/80" style={{ width: `${programmeProgress(onNow, now) * 100}%` }} />
                            </div>
                          </div>
                        )}
                      </div>
                      <button className="cv-btn cv-btn-primary px-2 py-1 text-[10px] opacity-0 group-hover:opacity-100">
                        <Play size={10} /> {active ? "Watching" : "Play"}
                      </button>
                    </div>
                  );
                })}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

const DEMO_PROFILES: LiveProfile[] = [
  {
    id: 1,
    name: "Premium IPTV",
    server_url: "http://iptv.example.com:8080",
    username: "user",
    password: "pass",
    enabled: true,
    last_synced: null,
    source_kind: "xtream",
  },
];
const DEMO_CHANNELS: LiveChannel[] = [
  { id: 1, profile_id: 1, name: "CNN", stream_url: "", logo_url: null, group_name: "News", epg_id: "cnn" },
  { id: 2, profile_id: 1, name: "ESPN", stream_url: "", logo_url: null, group_name: "Sports", epg_id: "espn" },
  { id: 3, profile_id: 1, name: "HBO", stream_url: "", logo_url: null, group_name: "Entertainment", epg_id: "hbo" },
];
