import { useEffect, useMemo, useRef, useState } from "react";
import type { JSX } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AnimatePresence, motion, useReducedMotion } from "framer-motion";
import {
  Activity,
  Bell,
  BrainCircuit,
  Cast,
  Cloud,
  Command,
  Cpu,
  Download,
  FolderOpen,
  Home,
  Maximize2,
  Puzzle,
  RadioTower,
  Router,
  Search,
  Server,
  Settings,
  Shield,
  Sliders,
  Sparkles,
  Tv,
  X,
  type LucideIcon,
} from "lucide-react";
import { useAppStore, type TabId } from "../store/appStore";
import { getUnreadStatusMessages } from "../utils/pluginUiSafety";

interface AppInfo {
  name: string;
  version: string;
  build: string;
  edition: string;
}

interface EmbeddedServerStatus {
  running: boolean;
  port: number;
}

interface RemoteConnectivityStatus {
  directAvailable: boolean;
  relayActive: boolean;
  relayMode?: string | null;
  preferredUrl?: string | null;
}

interface CommandDestination {
  id: TabId;
  label: string;
  description: string;
  icon: LucideIcon;
  keywords: string;
}

const FALLBACK_APP_INFO: AppInfo = {
  name: "CinaVault Premium",
  version: "1.7.170",
  build: "170",
  edition: "Premium",
};

const COMMAND_DESTINATIONS: CommandDestination[] = [
  {
    id: "home",
    label: "Library",
    description: "Browse, filter, play, verify, and manage media cards",
    icon: Home,
    keywords: "movies media home library posters cards play",
  },
  {
    id: "sources",
    label: "Media Sources",
    description: "Add local folders, scan libraries, and run enrichment",
    icon: FolderOpen,
    keywords: "folder drive scan source local import ingest",
  },
  {
    id: "downloads",
    label: "Downloads",
    description: "Manage acquisitions and incoming media",
    icon: Download,
    keywords: "download queue acquire youtube media",
  },
  {
    id: "livetv",
    label: "Live TV",
    description: "Open channels, guide data, and live streams",
    icon: Tv,
    keywords: "tv live channels epg guide stream",
  },
  {
    id: "server",
    label: "Server Core",
    description: "Inspect embedded media services and runtime health",
    icon: Server,
    keywords: "server core status services plex embedded",
  },
  {
    id: "remote",
    label: "Remote Access",
    description: "Manage NAT traversal, relay, and remote clients",
    icon: Router,
    keywords: "remote nat upnp relay cloud clients access",
  },
  {
    id: "security",
    label: "Security",
    description: "Control identities, VPN, scanning, and privacy",
    icon: Shield,
    keywords: "security vpn antivirus users privacy encryption",
  },
  {
    id: "ai",
    label: "AI Autopilot",
    description: "Automate metadata, posters, repairs, and diagnostics",
    icon: BrainCircuit,
    keywords: "ai autopilot metadata poster automation repair",
  },
  {
    id: "plugins",
    label: "Extensions",
    description: "Control metadata providers and media tooling",
    icon: Puzzle,
    keywords: "plugins extensions providers metadata tools",
  },
  {
    id: "cloud",
    label: "Cloud & NAS",
    description: "Connect storage services and network libraries",
    icon: Cloud,
    keywords: "cloud nas synology western digital storage",
  },
  {
    id: "advanced",
    label: "Advanced",
    description: "Open diagnostics and expert configuration",
    icon: Sliders,
    keywords: "advanced diagnostics debug expert tuning",
  },
  {
    id: "settings",
    label: "Settings",
    description: "Customize appearance, behavior, and preferences",
    icon: Settings,
    keywords: "settings themes appearance preferences",
  },
];

export default function Header(): JSX.Element {
  const {
    activeTab,
    setActiveTab,
    searchQuery,
    setSearchQuery,
    statusMessages,
    settings,
  } = useAppStore();
  const reduceMotion = useReducedMotion();
  const [clock, setClock] = useState(() => new Date());
  const [appInfo, setAppInfo] = useState<AppInfo>(FALLBACK_APP_INFO);
  const [showNotifications, setShowNotifications] = useState(false);
  const [lastReadMessageIndex, setLastReadMessageIndex] = useState(0);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [paletteQuery, setPaletteQuery] = useState("");
  const [serverStatus, setServerStatus] = useState<EmbeddedServerStatus>({
    running: false,
    port: 32400,
  });
  const [remoteStatus, setRemoteStatus] = useState<RemoteConnectivityStatus>({
    directAvailable: false,
    relayActive: false,
  });
  const paletteInputRef = useRef<HTMLInputElement | null>(null);

  const unreadMessages = useMemo(
    () => getUnreadStatusMessages(statusMessages, lastReadMessageIndex),
    [statusMessages, lastReadMessageIndex],
  );
  const filteredDestinations = useMemo(() => {
    const query = paletteQuery.trim().toLowerCase();
    if (!query) return COMMAND_DESTINATIONS;
    return COMMAND_DESTINATIONS.filter((item) =>
      `${item.label} ${item.description} ${item.keywords}`
        .toLowerCase()
        .includes(query),
    );
  }, [paletteQuery]);
  const autopilotEnabled = settings.ai_media_autopilot_enabled !== "false";
  const remoteLabel = remoteStatus.relayActive
    ? `${remoteStatus.relayMode || "Cloud"} relay`
    : remoteStatus.directAvailable
      ? "Direct route"
      : "Local only";

  useEffect(() => {
    let active = true;
    void invoke<AppInfo>("get_app_info")
      .then((info) => {
        if (active && info?.build) setAppInfo(info);
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    const timer = window.setInterval(() => setClock(new Date()), 1000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    const refresh = async () => {
      const [serverResult, remoteResult] = await Promise.allSettled([
        invoke<EmbeddedServerStatus>("get_embedded_server_status"),
        invoke<RemoteConnectivityStatus>("get_remote_connectivity_status"),
      ]);
      if (serverResult.status === "fulfilled") {
        setServerStatus(serverResult.value);
      }
      if (remoteResult.status === "fulfilled") {
        setRemoteStatus(remoteResult.value);
      }
    };
    void refresh();
    const timer = window.setInterval(refresh, 7000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    const handleKey = (event: KeyboardEvent) => {
      const key = event.key.toLowerCase();
      if ((event.ctrlKey || event.metaKey) && key === "k") {
        event.preventDefault();
        setPaletteOpen((open) => !open);
      }
      if (event.key === "Escape") {
        setPaletteOpen(false);
        setShowNotifications(false);
      }
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, []);

  useEffect(() => {
    if (!paletteOpen) return;
    const timer = window.setTimeout(() => paletteInputRef.current?.focus(), 50);
    return () => window.clearTimeout(timer);
  }, [paletteOpen]);

  const navigate = (tab: TabId) => {
    setActiveTab(tab);
    setPaletteOpen(false);
    setPaletteQuery("");
  };

  const toggleFullscreen = async () => {
    try {
      if (document.fullscreenElement) {
        await document.exitFullscreen();
      } else {
        await document.documentElement.requestFullscreen();
      }
    } catch (error) {
      console.warn("Fullscreen request failed:", error);
    }
  };

  return (
    <>
      <header className="relative z-30 shrink-0 px-4 pb-3 pt-3">
        <div className="flex min-h-[64px] items-center gap-3 rounded-[22px] border border-white/[0.10] bg-[linear-gradient(110deg,rgba(255,255,255,0.075),rgba(255,255,255,0.018)),rgba(3,6,18,0.72)] px-3 shadow-[inset_0_1px_0_rgba(255,255,255,0.10),0_14px_36px_rgba(0,0,0,0.26)] backdrop-blur-2xl">
          <motion.button
            type="button"
            onClick={() => setPaletteOpen(true)}
            whileHover={reduceMotion ? undefined : { scale: 1.018 }}
            whileTap={{ scale: 0.985 }}
            className="flex min-w-0 items-center gap-3 rounded-[17px] border border-cyan-200/14 bg-[linear-gradient(90deg,rgba(105,247,255,0.10),rgba(184,92,255,0.06))] px-3 py-2 text-left"
          >
            <span className="grid h-10 w-10 shrink-0 place-items-center rounded-[14px] border border-cyan-200/18 bg-black/25 text-cyan-100 shadow-[0_0_18px_rgba(105,247,255,0.12)]">
              <Command size={17} />
            </span>
            <span className="hidden min-w-0 sm:block">
              <span className="block text-[9px] font-black uppercase tracking-[0.24em] text-cyan-200/80">
                Command Deck
              </span>
              <span className="block truncate text-[13px] font-black text-white">
                {appInfo.name} · Build {appInfo.build}
              </span>
            </span>
            <span className="hidden rounded-lg border border-white/10 bg-black/25 px-2 py-1 text-[9px] font-black text-slate-400 lg:block">
              Ctrl K
            </span>
          </motion.button>

          <div className="relative min-w-0 flex-1">
            <Search
              size={15}
              className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-cyan-100/70"
            />
            <input
              type="text"
              value={searchQuery}
              onChange={(event) => setSearchQuery(event.target.value)}
              placeholder="Search the entire vault..."
              className="h-11 w-full rounded-[16px] border border-white/[0.08] bg-black/25 pl-10 pr-4 text-sm text-white outline-none transition placeholder:text-slate-500 focus:border-cyan-200/35 focus:bg-cyan-200/[0.04] focus:shadow-[0_0_24px_rgba(105,247,255,0.10)]"
              aria-label="Search the library"
            />
          </div>

          <div className="hidden items-center gap-2 xl:flex">
            <div className="flex items-center gap-2 rounded-[15px] border border-white/[0.08] bg-black/20 px-3 py-2">
              <span className="cv-status-orb" />
              <div>
                <div className="text-[10px] font-black text-white">
                  {serverStatus.running ? "Server online" : "Server starting"}
                </div>
                <div className="text-[8px] font-bold uppercase tracking-[0.16em] text-slate-500">
                  Port {serverStatus.port || 32400}
                </div>
              </div>
            </div>

            <div className="flex items-center gap-2 rounded-[15px] border border-white/[0.08] bg-black/20 px-3 py-2">
              <RadioTower size={14} className="text-fuchsia-300" />
              <div>
                <div className="text-[10px] font-black text-white">
                  {remoteLabel}
                </div>
                <div className="text-[8px] font-bold uppercase tracking-[0.16em] text-slate-500">
                  Remote path
                </div>
              </div>
            </div>

            <div className="flex items-center gap-2 rounded-[15px] border border-white/[0.08] bg-black/20 px-3 py-2">
              <BrainCircuit size={14} className="text-cyan-200" />
              <div>
                <div className="text-[10px] font-black text-white">
                  {autopilotEnabled ? "AI active" : "AI manual"}
                </div>
                <div className="text-[8px] font-bold uppercase tracking-[0.16em] text-slate-500">
                  Media autopilot
                </div>
              </div>
            </div>
          </div>

          <div className="hidden min-w-[74px] text-right font-mono text-[10px] tracking-wide text-slate-400 md:block">
            <div className="text-white">
              {clock.toLocaleTimeString([], {
                hour: "2-digit",
                minute: "2-digit",
              })}
            </div>
            <div>{clock.toLocaleDateString([], { month: "short", day: "2-digit" })}</div>
          </div>

          <button
            type="button"
            onClick={() =>
              window.dispatchEvent(new Event("cinavault:open-casting"))
            }
            className="grid h-11 w-11 shrink-0 place-items-center rounded-[15px] border border-fuchsia-200/16 bg-fuchsia-300/[0.07] text-fuchsia-100 transition hover:border-fuchsia-200/35 hover:bg-fuchsia-300/[0.13]"
            title="Open Casting Center"
          >
            <Cast size={16} />
          </button>

          <button
            type="button"
            onClick={() => void toggleFullscreen()}
            className="grid h-11 w-11 shrink-0 place-items-center rounded-[15px] border border-white/[0.08] bg-white/[0.035] text-slate-300 transition hover:border-cyan-200/25 hover:bg-cyan-200/[0.07] hover:text-white"
            title="Toggle fullscreen"
          >
            <Maximize2 size={16} />
          </button>

          <button
            type="button"
            onClick={() => {
              setShowNotifications((open) => !open);
              setLastReadMessageIndex(statusMessages.length);
            }}
            className="relative grid h-11 w-11 shrink-0 place-items-center rounded-[15px] border border-white/[0.08] bg-white/[0.035] text-slate-300 transition hover:border-cyan-200/25 hover:bg-cyan-200/[0.07] hover:text-white"
            title="Open command feed"
          >
            <Bell size={16} />
            {unreadMessages.length > 0 && (
              <span className="absolute right-2 top-2 h-2 w-2 rounded-full bg-amber-300 shadow-[0_0_12px_rgba(255,200,87,0.95)]" />
            )}
          </button>
        </div>

        <AnimatePresence>
          {showNotifications && (
            <motion.div
              initial={{ opacity: 0, y: -8, scale: 0.98 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: -8, scale: 0.98 }}
              className="absolute right-4 top-[82px] z-50 w-[420px] max-w-[calc(100vw-2rem)] overflow-hidden rounded-[22px] border border-cyan-200/18 bg-[rgba(4,7,19,0.96)] shadow-[0_28px_80px_rgba(0,0,0,0.62),0_0_46px_rgba(105,247,255,0.10)] backdrop-blur-3xl"
            >
              <div className="flex items-center justify-between border-b border-white/[0.08] px-4 py-3">
                <div className="flex items-center gap-2 text-xs font-black uppercase tracking-[0.15em] text-cyan-100">
                  <Activity size={14} /> Live Command Feed
                </div>
                <button
                  type="button"
                  onClick={() => setShowNotifications(false)}
                  className="grid h-8 w-8 place-items-center rounded-xl border border-white/[0.08] text-slate-400 hover:text-white"
                >
                  <X size={14} />
                </button>
              </div>
              <div className="max-h-[360px] overflow-y-auto p-2">
                {statusMessages.length === 0 ? (
                  <div className="p-5 text-sm text-slate-400">
                    System activity will appear here.
                  </div>
                ) : (
                  statusMessages
                    .slice(-16)
                    .reverse()
                    .map((message, index) => (
                      <motion.div
                        key={`${message}-${index}`}
                        initial={{ opacity: 0, x: 8 }}
                        animate={{ opacity: 1, x: 0 }}
                        transition={{ delay: index * 0.02 }}
                        className="mb-1 rounded-[14px] border border-white/[0.06] bg-white/[0.025] px-3 py-3 text-xs leading-relaxed text-slate-200 last:mb-0"
                      >
                        {message}
                      </motion.div>
                    ))
                )}
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </header>

      <AnimatePresence>
        {paletteOpen && (
          <motion.div
            className="cv-command-palette-backdrop"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onMouseDown={(event) => {
              if (event.target === event.currentTarget) setPaletteOpen(false);
            }}
          >
            <motion.div
              className="cv-command-palette"
              initial={{ opacity: 0, y: -18, scale: 0.96 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: -18, scale: 0.96 }}
              transition={{ duration: 0.2, ease: [0.16, 1, 0.3, 1] }}
            >
              <div className="relative">
                <Search
                  size={17}
                  className="pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-cyan-200"
                />
                <input
                  ref={paletteInputRef}
                  value={paletteQuery}
                  onChange={(event) => setPaletteQuery(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" && filteredDestinations[0]) {
                      navigate(filteredDestinations[0].id);
                    }
                  }}
                  className="cv-command-input pl-12"
                  placeholder="Go anywhere or search capabilities..."
                  aria-label="Command palette"
                />
              </div>
              <div className="max-h-[470px] overflow-y-auto p-2">
                {filteredDestinations.length === 0 ? (
                  <div className="p-7 text-center text-sm text-slate-400">
                    No matching destination.
                  </div>
                ) : (
                  filteredDestinations.map((destination) => {
                    const Icon = destination.icon;
                    return (
                      <button
                        key={destination.id}
                        type="button"
                        onClick={() => navigate(destination.id)}
                        className="cv-command-item rounded-[16px]"
                      >
                        <span className="cv-command-icon">
                          <Icon size={17} />
                        </span>
                        <span className="min-w-0 flex-1">
                          <span className="block text-sm font-black text-white">
                            {destination.label}
                          </span>
                          <span className="block truncate text-xs text-slate-400">
                            {destination.description}
                          </span>
                        </span>
                        {activeTab === destination.id && (
                          <span className="rounded-full border border-cyan-200/20 bg-cyan-200/[0.08] px-2 py-1 text-[9px] font-black uppercase tracking-[0.14em] text-cyan-100">
                            Active
                          </span>
                        )}
                      </button>
                    );
                  })
                )}
              </div>
              <div className="flex items-center justify-between border-t border-white/[0.08] px-4 py-3 text-[10px] font-bold uppercase tracking-[0.16em] text-slate-500">
                <span className="flex items-center gap-2">
                  <Sparkles size={12} className="text-fuchsia-300" /> Build 170 spatial command system
                </span>
                <span>Esc to close</span>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </>
  );
}
