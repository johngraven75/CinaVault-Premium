import CastButton from "./components/CastButton";
import "./styles/poster-card-standard.css";
import { AI_MEDIA_AGENT_ENABLED } from "./services/aiMediaAgent";
import { getPreferredMediaServer } from "./services/serverProvider";
import { getEnabledCinaVaultFeatures } from "./features/cinavaultFeatureSuite";
import "./styles/media-row-poster-final-fix.css";
// CinaVault Premium — Build 140 Futuristic Application Shell
import { useEffect, useCallback, useRef } from "react";
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
import { getWheelDeltaPixels, getWheelScrolledTop } from "./utils/pageWheelScroll";
import "./styles/media-card-hard-fix.css";

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

const TAB_MOTION = {
  initial: { opacity: 0, y: 18, scale: 0.985, filter: "blur(8px)" },
  animate: { opacity: 1, y: 0, scale: 1, filter: "blur(0px)" },
  exit: { opacity: 0, y: -16, scale: 0.992, filter: "blur(8px)" },
};

function findScrollableAncestor(target: Element, root: HTMLElement): HTMLElement {
  let node: HTMLElement | null = target instanceof HTMLElement ? target : target.parentElement;

  while (node && node !== root) {
    const style = window.getComputedStyle(node);
    if (/(auto|scroll)/.test(style.overflowY) && node.scrollHeight > node.clientHeight) {
      return node;
    }
    node = node.parentElement;
  }

  return root;
}

function canScrollInDirection(element: HTMLElement, deltaPixels: number): boolean {
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
    return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? parsed : {};
  } catch {
    return {};
  }
}

async function saveAllSettingsToBackend(state: Record<string, string>): Promise<void> {
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
    const deltaPixels = getWheelDeltaPixels(event.deltaY, event.deltaMode, root.clientHeight);

    if (!canScrollInDirection(scrollable, deltaPixels)) {
      const parentScroll = findScrollableAncestor(scrollable.parentElement || root, root);
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
    <div className="app-shell cv-app min-h-screen flex overflow-hidden" style={{ background: "var(--cv-bg-primary)" }}>
      <div className="app-shell-orb app-shell-orb-a" />
      <div className="app-shell-orb app-shell-orb-b" />
      <div className="app-shell-orb app-shell-orb-c" />
      <div className="app-shell-noise" />

      <motion.div
        className="relative z-10 flex h-screen w-full overflow-hidden p-3 gap-3"
        initial={{ opacity: 0, scale: 0.992 }}
        animate={{ opacity: 1, scale: 1 }}
        transition={{ duration: 0.45, ease: [0.16, 1, 0.3, 1] }}
      >
        <Sidebar />

        <CastButton />
      <div data-testid="cinavault-feature-suite" style={{ display: "none" }}>{getEnabledCinaVaultFeatures().length} enabled media server features</div>
      <div data-testid="cinavault-proprietary-server" style={{ display: "none" }}>{getPreferredMediaServer().primary}</div>
      <div data-testid="cinavault-ai-media-agent" style={{ display: "none" }}>{AI_MEDIA_AGENT_ENABLED ? "AI Media Agent Enabled" : "AI Media Agent Disabled"}</div>
      <main className="relative flex-1 flex flex-col overflow-hidden rounded-2xl border border-white/10 bg-black/15 shadow-[0_30px_80px_rgba(0,0,0,0.42)] backdrop-blur-xl">
          <div className="pointer-events-none absolute inset-0 rounded-2xl bg-[radial-gradient(circle_at_18%_0%,rgba(255,255,255,0.16),transparent_32%),radial-gradient(circle_at_100%_18%,rgba(0,234,255,0.13),transparent_30%)]" />
          <div className="pointer-events-none absolute inset-x-8 top-0 h-px bg-gradient-to-r from-transparent via-white/45 to-transparent" />

          <Header />

          <div
            ref={mainScrollRef}
            className="app-main-scroll relative z-10 flex-1 overflow-y-auto overflow-x-hidden px-4 pb-4 pt-3"
            onWheel={handleWheel}
          >
            <AnimatePresence mode="wait">
              <motion.section
                key={activeTab}
                initial={TAB_MOTION.initial}
                animate={TAB_MOTION.animate}
                exit={TAB_MOTION.exit}
                transition={{ duration: 0.32, ease: [0.16, 1, 0.3, 1] }}
                className="relative min-h-full rounded-2xl border border-white/[0.08] bg-[linear-gradient(145deg,rgba(255,255,255,0.055),rgba(255,255,255,0.018))] p-5 shadow-[inset_0_1px_0_rgba(255,255,255,0.12)] backdrop-blur-md"
              >
                <div className="pointer-events-none absolute inset-x-6 top-0 h-px bg-gradient-to-r from-transparent via-[var(--cv-accent)]/55 to-transparent" />
                {CurrentTabComponent ? <CurrentTabComponent /> : <div>Tab not found</div>}
              </motion.section>
            </AnimatePresence>
          </div>
        </main>
      </motion.div>
    </div>
  );
}
