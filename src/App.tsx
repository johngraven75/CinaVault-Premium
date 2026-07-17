// Build 140 Futuristic Application Shell
import CastButton from "./components/CastButton";
import "./styles/poster-card-standard.css";
import { AI_MEDIA_AGENT_ENABLED } from "./services/aiMediaAgent";
import { getPreferredMediaServer } from "./services/serverProvider";
import { getEnabledCinaVaultFeatures } from "./features/cinavaultFeatureSuite";
import "./styles/media-row-poster-final-fix.css";
import {
  ensurePermanentMediaPluginsAtStartup,
  initializePermanentMediaPluginsAtStartup,
} from "./services/startupMediaPluginService";
// CinaVault Premium — Build 155 Media Center Application Shell
import { useEffect, useCallback, useMemo, useRef } from "react";
import type { FC, JSX, WheelEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { motion, AnimatePresence } from "framer-motion";
import { useAppStore, TabId } from "./store/appStore";
import { applyTheme } from "./themes";
import "./data/pluginAdapterInitialize";
import Sidebar from "./components/Sidebar";
import Header from "./components/Header";
import HomeTab from "./components/tabs/HomeTab";
import MediaSourcesTab from "./components/tabs/MediaSourcesTab";
import DownloadsTab from "./components/tabs/DownloadsTab";
import LiveTVTab from "./components/tabs/LiveTVTab";
import ServerTab from "./components/tabs/ServerTab";
import SecurityTab from "./components/tabs/SecurityTab";
import RemoteAccessTab from "./components/tabs/RemoteAccessTab";
import AdvancedTab from "./components/tabs/AdvancedTab";
import CloudNASTab from "./components/tabs/CloudNASTab";
import PluginsTab from "./components/tabs/PluginsTab";
import AIDiagnosticsTab from "./components/tabs/AIDiagnosticsTab";
import SettingsTab from "./components/tabs/SettingsTab";
import { pluginEngine } from "./data/pluginAdapter";
import {
  getWheelDeltaPixels,
  getWheelScrolledTop,
} from "./utils/pageWheelScroll";
import "./styles/media-card-hard-fix.css";
import "./styles/media-card-final-standard.css";

const TAB_COMPONENTS: Record<TabId, FC> = {
  home: HomeTab,
  sources: MediaSourcesTab,
  downloads: DownloadsTab,
  livetv: LiveTVTab,
  server: ServerTab,
  security: SecurityTab,
  remote: RemoteAccessTab,
  advanced: AdvancedTab,
  cloud: CloudNASTab,
  plugins: PluginsTab,
  ai: AIDiagnosticsTab,
  settings: SettingsTab,
};

const TAB_TITLES: Record<
  TabId,
  { eyebrow: string; title: string; subtitle: string }
> = {
  home: {
    eyebrow: "Media Center",
    title: "Home",
    subtitle:
      "Featured library shelves, recent additions, and playback entry points.",
  },
  sources: {
    eyebrow: "Library",
    title: "Media Sources",
    subtitle: "Add, scan, repair, and verify source folders.",
  },
  downloads: {
    eyebrow: "Queue",
    title: "Downloads",
    subtitle: "Track incoming media and library acquisition tasks.",
  },
  livetv: {
    eyebrow: "Channels",
    title: "Live TV",
    subtitle: "Live streams, guide data, and viewing controls.",
  },
  server: {
    eyebrow: "Core",
    title: "Server",
    subtitle: "CinaVault server status, services, and media delivery controls.",
  },
  security: {
    eyebrow: "Guard",
    title: "Security",
    subtitle: "Protect access, credentials, and runtime configuration.",
  },
  remote: {
    eyebrow: "Access",
    title: "Remote Access",
    subtitle: "External connection and relay configuration.",
  },
  advanced: {
    eyebrow: "Tools",
    title: "Advanced",
    subtitle: "Power-user repair, diagnostics, and build controls.",
  },
  cloud: {
    eyebrow: "Storage",
    title: "Cloud & NAS",
    subtitle: "Network storage, cloud library paths, and sync options.",
  },
  plugins: {
    eyebrow: "Add-ons",
    title: "Plugins",
    subtitle: "Installed media helpers and permanent startup plugin coverage.",
  },
  ai: {
    eyebrow: "Assistant",
    title: "AI Diagnostics",
    subtitle:
      "Inference, provider checks, metadata posting, and guided repairs.",
  },
  settings: {
    eyebrow: "Config",
    title: "Settings",
    subtitle: "Theme, application, and persistent user preferences.",
  },
};

const TAB_MOTION = {
  initial: { opacity: 0, y: 26, scale: 0.982, filter: "blur(10px)" },
  animate: { opacity: 1, y: 0, scale: 1, filter: "blur(0px)" },
  exit: { opacity: 0, y: -18, scale: 0.99, filter: "blur(10px)" },
};

function findScrollableAncestor(
  target: Element,
  root: HTMLElement,
): HTMLElement {
  let node: HTMLElement | null =
    target instanceof HTMLElement ? target : target.parentElement;

  while (node && node !== root) {
    const style = window.getComputedStyle(node);
    if (
      /(auto|scroll)/.test(style.overflowY) &&
      node.scrollHeight > node.clientHeight
    ) {
      return node;
    }
    node = node.parentElement;
  }

  return root;
}

function canScrollInDirection(
  element: HTMLElement,
  deltaPixels: number,
): boolean {
  if (deltaPixels > 0) {
    return element.scrollTop + element.clientHeight < element.scrollHeight - 1;
  }

  if (deltaPixels < 0) {
    return element.scrollTop > 0;
  }

  return false;
}

function readLocalPersistedState(): Record<string, string> {
  try {
    const raw = localStorage.getItem("cinavault_state");
    if (!raw) return {};
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? parsed
      : {};
  } catch {
    return {};
  }
}

async function saveAllSettingsToBackend(
  state: Record<string, string>,
): Promise<void> {
  try {
    for (const [key, value] of Object.entries(state)) {
      await invoke("set_setting", { key, value });
    }
  } catch {
    try {
      localStorage.setItem("cinavault_state", JSON.stringify(state));
    } catch (storageError) {
      console.warn("Failed to save state:", storageError);
    }
  }
}

export default function App(): JSX.Element {
  const {
    activeTab,
    currentTheme,
    sidebarCollapsed,
    addStatusMessage,
    getPersistedState,
    restorePersistedState,
  } = useAppStore();

  const isSaving = useRef<boolean>(false);
  const mainScrollRef = useRef<HTMLDivElement | null>(null);
  const activeTitle = TAB_TITLES[activeTab];
  const featureCount = useMemo(() => getEnabledCinaVaultFeatures().length, []);
  const serverName = useMemo(() => getPreferredMediaServer().primary, []);
  const startupPluginsReady = initializePermanentMediaPluginsAtStartup().ready;

  const saveState = useCallback(async (): Promise<void> => {
    if (isSaving.current) return;

    isSaving.current = true;
    try {
      const state = getPersistedState();
      await saveAllSettingsToBackend(state);
    } catch (error) {
      console.error("Save state error:", error);
      addStatusMessage("Failed to save settings");
    } finally {
      isSaving.current = false;
    }
  }, [getPersistedState, addStatusMessage]);

  useEffect(() => {
    let cancelled = false;

    const initializeApplication = async (): Promise<void> => {
      try {
        restorePersistedState(readLocalPersistedState());
        if (cancelled) return;

        applyTheme(currentTheme);
        await pluginEngine.initialize();
        const mediaTools = await ensurePermanentMediaPluginsAtStartup();
        if (!mediaTools.ready) {
          const missing = mediaTools.tools
            .filter((tool) => !tool.installed)
            .map((tool) => tool.id)
            .join(", ");
          addStatusMessage(
            `Automatic media-tool setup needs attention: ${missing || "unknown tools"}`,
          );
        } else {
          addStatusMessage("FFmpeg, FFprobe, yt-dlp, MediaInfo, and MKVToolNix loaded");
        }

        const appWindow = getCurrentWindow();
        await appWindow.setTitle("CinaVault Premium");
      } catch (error) {
        console.error("Initialization error:", error);
        addStatusMessage("Failed to initialize application");
      }
    };

    void initializeApplication();

    return () => {
      cancelled = true;
    };
  }, [restorePersistedState, currentTheme, addStatusMessage]);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void saveState();
    }, 1000);

    return () => window.clearTimeout(timer);
  }, [activeTab, currentTheme, sidebarCollapsed, saveState]);

  const handleWheel = useCallback((event: WheelEvent<HTMLDivElement>): void => {
    if (!(event.target instanceof Element)) return;

    const root = mainScrollRef.current;
    if (!root) return;

    const scrollable = findScrollableAncestor(event.target, root);
    const deltaPixels = getWheelDeltaPixels(
      event.deltaY,
      event.deltaMode,
      root.clientHeight,
    );

    if (!canScrollInDirection(scrollable, deltaPixels)) {
      const parentScroll = findScrollableAncestor(
        scrollable.parentElement || root,
        root,
      );
      if (canScrollInDirection(parentScroll, deltaPixels)) {
        parentScroll.scrollTop = getWheelScrolledTop(
          parentScroll.scrollTop,
          deltaPixels,
          parentScroll.scrollHeight,
          parentScroll.clientHeight,
        );
        event.preventDefault();
      }
    }
  }, []);

  const CurrentTabComponent = TAB_COMPONENTS[activeTab];

  return (
    <div className="app-shell cv-app min-h-screen flex overflow-hidden bg-[#030813] text-cv-text">
      <div className="absolute inset-0 bg-[radial-gradient(circle_at_18%_0%,rgba(0,170,255,0.28),transparent_34%),radial-gradient(circle_at_92%_18%,rgba(255,255,255,0.11),transparent_30%),linear-gradient(180deg,#06111f_0%,#030813_54%,#020409_100%)]" />
      <div className="app-shell-orb app-shell-orb-a" />
      <div className="app-shell-orb app-shell-orb-b" />
      <div className="app-shell-orb app-shell-orb-c" />
      <div className="app-shell-noise" />

      <motion.div
        className="relative z-10 flex h-screen w-full overflow-hidden p-4 gap-4"
        initial={{ opacity: 0, scale: 0.992 }}
        animate={{ opacity: 1, scale: 1 }}
        transition={{ duration: 0.45, ease: [0.16, 1, 0.3, 1] }}
      >
        <Sidebar />

        <CastButton />
        <div
          data-testid="cinavault-permanent-media-plugins"
          style={{ display: "none" }}
        >
          {startupPluginsReady
            ? "Permanent media plugins ready"
            : "Permanent media plugins not ready"}
        </div>
        <div data-testid="cinavault-feature-suite" style={{ display: "none" }}>
          {featureCount} enabled media server features
        </div>
        <div
          data-testid="cinavault-proprietary-server"
          style={{ display: "none" }}
        >
          {serverName}
        </div>
        <div data-testid="cinavault-ai-media-agent" style={{ display: "none" }}>
          {AI_MEDIA_AGENT_ENABLED
            ? "AI Media Agent Enabled"
            : "AI Media Agent Disabled"}
        </div>

        <main className="relative flex-1 flex flex-col overflow-hidden rounded-[32px] border border-white/10 bg-[#06101d]/82 shadow-[0_30px_100px_rgba(0,0,0,0.62)] backdrop-blur-xl">
          <div className="pointer-events-none absolute inset-0 rounded-[32px] bg-[radial-gradient(circle_at_15%_0%,rgba(255,255,255,0.17),transparent_35%),radial-gradient(circle_at_100%_18%,rgba(0,234,255,0.15),transparent_30%)]" />
          <div className="pointer-events-none absolute inset-x-10 top-0 h-px bg-gradient-to-r from-transparent via-white/45 to-transparent" />

          <Header />

          <section className="relative z-10 mx-4 mt-2 overflow-hidden rounded-[28px] border border-white/10 bg-[linear-gradient(105deg,rgba(0,0,0,0.48),rgba(255,255,255,0.08)),radial-gradient(circle_at_86%_22%,rgba(0,234,255,0.22),transparent_28%)] p-5 shadow-[inset_0_1px_0_rgba(255,255,255,0.12)]">
            <div className="absolute inset-0 bg-[linear-gradient(90deg,rgba(3,8,19,0.95)_0%,rgba(3,8,19,0.68)_48%,rgba(3,8,19,0.28)_100%)]" />
            <div className="relative z-10 grid grid-cols-1 gap-4 lg:grid-cols-[minmax(0,1fr)_360px] lg:items-end">
              <div>
                <div className="text-[11px] font-bold uppercase tracking-[0.34em] text-cyan-200">
                  {activeTitle.eyebrow}
                </div>
                <h1 className="mt-2 text-4xl font-black tracking-tight md:text-5xl">
                  {activeTitle.title}
                </h1>
                <p className="mt-2 max-w-3xl text-sm text-cv-subtext md:text-base">
                  {activeTitle.subtitle}
                </p>
              </div>
              <div className="grid grid-cols-3 gap-2 text-center">
                <div className="rounded-2xl border border-white/10 bg-black/25 p-3">
                  <div className="text-2xl font-black text-cv-text">
                    {featureCount}
                  </div>
                  <div className="text-[9px] uppercase tracking-[0.22em] text-cv-subtext">
                    Features
                  </div>
                </div>
                <div className="rounded-2xl border border-white/10 bg-black/25 p-3">
                  <div className="text-2xl font-black text-cv-text">
                    {startupPluginsReady ? "On" : "Off"}
                  </div>
                  <div className="text-[9px] uppercase tracking-[0.22em] text-cv-subtext">
                    Plugins
                  </div>
                </div>
                <div className="rounded-2xl border border-white/10 bg-black/25 p-3">
                  <div className="truncate text-lg font-black text-cv-text">
                    {serverName}
                  </div>
                  <div className="text-[9px] uppercase tracking-[0.22em] text-cv-subtext">
                    Server
                  </div>
                </div>
              </div>
            </div>
          </section>

          <div
            ref={mainScrollRef}
            className="app-main-scroll relative z-10 flex-1 overflow-y-auto overflow-x-hidden px-4 pb-4 pt-4"
            onWheel={handleWheel}
          >
            <AnimatePresence mode="wait">
              <motion.section
                key={activeTab}
                initial={TAB_MOTION.initial}
                animate={TAB_MOTION.animate}
                exit={TAB_MOTION.exit}
                transition={{ duration: 0.32, ease: [0.16, 1, 0.3, 1] }}
                className="relative min-h-full rounded-[28px] border border-white/[0.08] bg-[linear-gradient(145deg,rgba(255,255,255,0.06),rgba(255,255,255,0.02))] p-5 shadow-[inset_0_1px_0_rgba(255,255,255,0.12)] backdrop-blur-md"
              >
                <div className="pointer-events-none absolute inset-x-6 top-0 h-px bg-gradient-to-r from-transparent via-[var(--cv-accent)]/55 to-transparent" />
                {CurrentTabComponent ? (
                  <CurrentTabComponent />
                ) : (
                  <div>Tab not found</div>
                )}
              </motion.section>
            </AnimatePresence>
          </div>
        </main>
      </motion.div>
    </div>
  );
}
