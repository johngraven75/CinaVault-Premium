// CinaVault Premium — runtime-driven Cyber HUD Command Header
import { useEffect, useMemo, useState } from "react";
import type { JSX } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AnimatePresence, motion, useReducedMotion } from "framer-motion";
import {
  Activity,
  Bell,
  Cpu,
  Maximize2,
  RadioTower,
  Search,
  ShieldCheck,
  Sparkles,
  Zap,
} from "lucide-react";
import { useAppStore, type TabId } from "../store/appStore";
import { getUnreadStatusMessages } from "../utils/pluginUiSafety";

interface AppInfo {
  name: string;
  version: string;
  build: string;
  edition: string;
}

interface TabMeta {
  label: string;
  subtitle: string;
  signal: string;
}

const FALLBACK_APP_INFO: AppInfo = {
  name: "CinaVault Premium",
  version: "1.7.1",
  build: "171",
  edition: "Premium",
};

const TAB_META: Record<TabId, TabMeta> = {
  home: {
    label: "Movies",
    subtitle: "Holographic library carousel, vault inventory, and instant playback",
    signal: "Vault",
  },
  sources: {
    label: "Media Sources",
    subtitle: "Ingest folders, drives, network shares, and scan targets",
    signal: "Ingest",
  },
  downloads: {
    label: "My Vault",
    subtitle: "Queue telemetry, acquisitions, and personal watch staging",
    signal: "Queue",
  },
  livetv: {
    label: "TV Shows",
    subtitle: "Live streams, channel intelligence, and guide signals",
    signal: "Stream",
  },
  server: {
    label: "System Core",
    subtitle: "Services, runtime health, networking, and uptime telemetry",
    signal: "Core",
  },
  security: {
    label: "Security",
    subtitle: "Access control, privacy shields, audit trails, and hardening",
    signal: "Guard",
  },
  remote: {
    label: "Remote",
    subtitle: "Relay state, external reachability, and secure remote paths",
    signal: "Relay",
  },
  advanced: {
    label: "Advanced",
    subtitle: "Expert tuning, debug controls, and platform diagnostics",
    signal: "Tune",
  },
  cloud: {
    label: "Cloud / NAS",
    subtitle: "Cloud sync, NAS fabric, and storage mesh control",
    signal: "Mesh",
  },
  plugins: {
    label: "Plugins",
    subtitle: "Metadata engines, compatibility bridges, and extension control",
    signal: "Mods",
  },
  ai: {
    label: "AI Terminal",
    subtitle: "Predictive diagnostics, repair guidance, and neural analysis",
    signal: "Neural",
  },
  settings: {
    label: "Settings",
    subtitle: "Themes, preferences, profiles, and cinematic behavior",
    signal: "Config",
  },
};

const PRIMARY_TABS: TabId[] = [
  "home",
  "livetv",
  "downloads",
  "sources",
  "server",
  "plugins",
  "ai",
  "settings",
];

export default function Header(): JSX.Element {
  const {
    activeTab,
    setActiveTab,
    searchQuery,
    setSearchQuery,
    statusMessages,
  } = useAppStore();
  const reduceMotion = useReducedMotion();
  const [clock, setClock] = useState(() => new Date());
  const [showNotifications, setShowNotifications] = useState(false);
  const [lastReadMessageIndex, setLastReadMessageIndex] = useState(0);
  const [appInfo, setAppInfo] = useState<AppInfo>(FALLBACK_APP_INFO);

  const unreadMessages = useMemo(
    () => getUnreadStatusMessages(statusMessages, lastReadMessageIndex),
    [statusMessages, lastReadMessageIndex],
  );
  const activeMeta = TAB_META[activeTab] ?? TAB_META.home;

  useEffect(() => {
    let active = true;
    void invoke<AppInfo>("get_app_info")
      .then((info) => {
        if (active && info?.build) setAppInfo(info);
      })
      .catch((error: unknown) => {
        console.warn("Runtime build identity could not be loaded:", error);
      });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    const timer = window.setInterval(() => setClock(new Date()), 1000);
    return () => window.clearInterval(timer);
  }, []);

  const toggleFullscreen = async (): Promise<void> => {
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
    <header className="cyber-header relative z-20 shrink-0 overflow-visible">
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_16%_25%,rgba(0,245,255,0.16),transparent_34%),linear-gradient(90deg,rgba(5,5,10,0.92),rgba(5,12,24,0.82))]" />

      <div className="relative z-10 flex h-full flex-col gap-3 px-4 py-3 xl:px-5">
        <div className="flex min-h-0 items-center justify-between gap-4">
          <div className="flex min-w-0 items-center gap-3">
            <motion.div
              className="cyber-brand-chip h-12 px-3"
              whileHover={reduceMotion ? undefined : { scale: 1.02 }}
              transition={{ duration: 0.16 }}
              title={`${appInfo.name} ${appInfo.version} Build ${appInfo.build}`}
            >
              <Zap size={18} className="text-cyan-200" />
              <div className="min-w-0">
                <div className="text-[10px] font-black uppercase tracking-[0.28em] text-cyan-200">
                  CinaVault B{appInfo.build}
                </div>
                <div className="truncate text-sm font-black uppercase tracking-[0.16em]">
                  {appInfo.edition} · v{appInfo.version}
                </div>
              </div>
            </motion.div>

            <motion.div
              key={activeTab}
              initial={reduceMotion ? false : { opacity: 0, y: 5 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: reduceMotion ? 0 : 0.14 }}
              className="hidden min-w-0 md:block"
            >
              <div className="cyber-eyebrow flex items-center gap-2">
                <Sparkles size={12} /> Quantum Grid Active / {activeMeta.signal}
              </div>
              <h1 className="cyber-title truncate text-xl font-black tracking-tight">
                {activeMeta.label}
              </h1>
              <p className="truncate text-xs text-cv-subtext">
                {activeMeta.subtitle}
              </p>
            </motion.div>
          </div>

          <div className="flex shrink-0 items-center gap-2 lg:gap-3">
            <div className="hidden items-center gap-2 border border-cyan-300/20 bg-black/40 px-3 py-2 text-[10px] font-black uppercase tracking-[0.18em] text-cyan-200 lg:flex">
              <Activity size={13} className="text-emerald-300" />
              Nominal
            </div>

            <div className="cyber-search-core hidden sm:block">
              <Search
                size={15}
                className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-cyan-200"
              />
              <input
                type="text"
                value={searchQuery}
                onChange={(event) => setSearchQuery(event.target.value)}
                placeholder="Search media and tools..."
                className="cyber-search-input"
                aria-label="Search library, plugins, and sources"
              />
            </div>

            <div className="hidden min-w-[94px] text-right font-mono text-[11px] tracking-wide text-cv-subtext/90 md:block">
              {clock.toLocaleTimeString([], {
                hour: "2-digit",
                minute: "2-digit",
                second: "2-digit",
              })}
            </div>

            <button
              type="button"
              onClick={() => void toggleFullscreen()}
              className="cyber-button h-11 w-11 px-0"
              title="Toggle fullscreen"
              aria-label="Toggle fullscreen"
            >
              <Maximize2 size={15} />
            </button>

            <button
              type="button"
              onClick={() => {
                setShowNotifications((open) => !open);
                setLastReadMessageIndex(statusMessages.length);
              }}
              className="cyber-button relative h-11 w-11 px-0"
              title="Show command feed"
              aria-label="Show command feed"
            >
              <Bell size={15} />
              {unreadMessages.length > 0 && (
                <span className="absolute right-2 top-2 h-2 w-2 rounded-full bg-[var(--cyber-amber)] shadow-[0_0_14px_rgba(255,153,0,0.95)]" />
              )}
            </button>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <nav className="quantum-nav flex-1" aria-label="Primary CinaVault navigation">
            {PRIMARY_TABS.map((tab) => {
              const meta = TAB_META[tab];
              const isActive = activeTab === tab;
              return (
                <button
                  key={tab}
                  type="button"
                  data-label={meta.label}
                  className={`quantum-tab ${isActive ? "is-active" : ""}`}
                  onClick={() => setActiveTab(tab)}
                  aria-current={isActive ? "page" : undefined}
                >
                  {meta.label}
                </button>
              );
            })}
          </nav>

          <div className="hidden items-center gap-2 text-[10px] font-black uppercase tracking-[0.18em] text-cv-subtext xl:flex">
            <Cpu size={13} className="text-cyan-200" />
            HUD Link
            <ShieldCheck size={13} className="text-emerald-300" />
            Secure
          </div>
        </div>
      </div>

      <AnimatePresence>
        {showNotifications && (
          <motion.div
            initial={reduceMotion ? false : { opacity: 0, y: -6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -6 }}
            transition={{ duration: reduceMotion ? 0 : 0.14 }}
            className="cyber-terminal-panel absolute right-5 top-[calc(100%+10px)] z-50 w-96 max-w-[calc(100vw-2rem)] bg-[#05050a]/95 p-0"
          >
            <div className="flex items-center justify-between border-b border-cyan-300/15 px-4 py-3">
              <span className="flex items-center gap-2 text-xs font-black uppercase tracking-[0.16em] text-cyan-200">
                <RadioTower size={14} /> Command Feed
              </span>
              <span className="text-[10px] uppercase tracking-[0.18em] text-cv-subtext">
                {statusMessages.length} messages
              </span>
            </div>
            <div className="max-h-80 overflow-y-auto py-1">
              {statusMessages.length === 0 ? (
                <div className="px-4 py-5 text-xs text-cv-subtext">
                  No notifications yet
                </div>
              ) : (
                statusMessages
                  .slice(-12)
                  .reverse()
                  .map((message, index) => (
                    <div
                      key={`${message}-${index}`}
                      className="border-b border-cyan-300/[0.07] px-4 py-3 last:border-b-0"
                    >
                      <div className="text-xs leading-relaxed text-cv-text">
                        {message}
                      </div>
                    </div>
                  ))
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </header>
  );
}
