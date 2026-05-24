// CinaVault Premium — Main Application Shell (with Persistent Settings)
import React, { useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { motion, AnimatePresence } from "framer-motion";
import { useAppStore, TabId } from "./store/appStore";
import { applyTheme } from "./themes";
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
import DuplicateToolsTab from "./components/tabs/DuplicateToolsTab";
import PluginsTab from "./components/tabs/PluginsTab";
import AIDiagnosticsTab from "./components/tabs/AIDiagnosticsTab";
import SettingsTab from "./components/tabs/SettingsTab";
import { pluginEngine } from "./data/pluginAdapter";
import { getWheelDeltaPixels, getWheelScrolledTop } from "./utils/pageWheelScroll";

const TAB_COMPONENTS: Record<TabId, React.FC> = {
  home: HomeTab,
  sources: MediaSourcesTab,
  downloads: DownloadsTab,
  livetv: LiveTVTab,
  server: ServerTab,
  security: SecurityTab,
  remote: RemoteAccessTab,
  advanced: AdvancedTab,
  cloud: CloudNASTab,
  duplicates: DuplicateToolsTab,
  plugins: PluginsTab,
  ai: AIDiagnosticsTab,
  settings: SettingsTab,
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

// ── Save all settings to Tauri backend ──
async function saveAllSettingsToBackend(state: Record<string, string>) {
  try {
    const entries = Object.entries(state);
    for (const [key, value] of entries) {
      await invoke("set_setting", { key, value });
    }
  } catch {
    // Dev mode fallback — save to localStorage
    try { localStorage.setItem("cinavault_state", JSON.stringify(state)); } catch {}
  }
}

export default function App() {
  const {
    activeTab, currentTheme, sidebarCollapsed,
    setSettings, setTheme, addStatusMessage,
    getPersistedState, restorePersistedState,
  } = useAppStore();

  const isSaving = useRef(false);
  const mainScrollRef = useRef<HTMLElement | null>(null);

  // ── Persist: Save all settings on demand ──
  const saveState = useCallback(async () => {
    if (isSaving.current) return;
    isSaving.current = true;
    try {
      const state = useAppStore.getState().getPersistedState();
      await saveAllSettingsToBackend(state);
    } catch {}
    isSaving.current = false;
  }, []);

  // ── Load & Restore on mount ──
  useEffect(() => {
    const init = async () => {
      try {
        const settings = await invoke<Record<string, string>>("get_all_settings");
        // Restore full persisted state (tabs, providers, schedules, etc.)
        useAppStore.getState().restorePersistedState(settings);
        const theme = settings._currentTheme || settings.theme || "vidhub_flagship";
        useAppStore.getState().setTheme(theme);
        applyTheme(theme);
        addStatusMessage("Settings restored — All features active");
      } catch {
        // Dev mode — try localStorage
        try {
          const saved = localStorage.getItem("cinavault_state");
          if (saved) {
            const parsed = JSON.parse(saved);
            useAppStore.getState().restorePersistedState(parsed);
            const theme = parsed._currentTheme || parsed.theme || "vidhub_flagship";
            useAppStore.getState().setTheme(theme);
            applyTheme(theme);
          } else {
            applyTheme("vidhub_flagship");
          }
        } catch {
          applyTheme("vidhub_flagship");
        }
        addStatusMessage("Running in development mode — Premium Edition");
      }
    };
    init();
  }, []);

  // ── Preload installed plugin state at launch (MS-B/MS-A/MS-C) ──
  useEffect(() => {
    const preloadPlugins = async () => {
      await pluginEngine.loadFromBackend();
      addStatusMessage(`Plugin catalog ready at launch (${pluginEngine.getInstalled().length} installed)`);
    };
    preloadPlugins();
  }, [addStatusMessage]);

  // ── Save on window close (Tauri) ──
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    const setup = async () => {
      try {
        const appWindow = getCurrentWindow();
        unlisten = await appWindow.onCloseRequested(async (event) => {
          // Save all state before closing
          await saveState();
          // Allow close to proceed
        });
      } catch {
        // Not running in Tauri — use beforeunload
      }
    };
    setup();

    // Also save on browser beforeunload (dev mode)
    const handleBeforeUnload = () => {
      const state = useAppStore.getState().getPersistedState();
      try { localStorage.setItem("cinavault_state", JSON.stringify(state)); } catch {}
    };
    window.addEventListener("beforeunload", handleBeforeUnload);

    return () => {
      if (unlisten) unlisten();
      window.removeEventListener("beforeunload", handleBeforeUnload);
    };
  }, [saveState]);

  // ── Auto-save periodically (every 60s) ──
  useEffect(() => {
    const interval = setInterval(() => {
      saveState();
    }, 60000);
    return () => clearInterval(interval);
  }, [saveState]);

  // Apply theme when it changes
  useEffect(() => {
    applyTheme(currentTheme);
  }, [currentTheme]);

  const ActiveComponent = TAB_COMPONENTS[activeTab] || HomeTab;

  const handlePageWheel = useCallback((event: WheelEvent) => {
    const main = mainScrollRef.current;
    if (!main) return;
    const deltaPixels = getWheelDeltaPixels(event.deltaY, event.deltaMode, main.clientHeight);
    if (deltaPixels === 0) return;

    const target = event.target instanceof Element ? event.target : main;
    const scrollTarget = findScrollableAncestor(target, main);
    if (scrollTarget !== main && canScrollInDirection(scrollTarget, deltaPixels)) {
      return;
    }

    const nextTop = getWheelScrolledTop(main.scrollTop, deltaPixels, main.scrollHeight, main.clientHeight);
    if (nextTop !== main.scrollTop) {
      event.preventDefault();
      main.scrollTop = nextTop;
    }
  }, []);

  useEffect(() => {
    const main = mainScrollRef.current;
    if (!main) return;
    main.addEventListener("wheel", handlePageWheel, { passive: false });
    return () => main.removeEventListener("wheel", handlePageWheel);
  }, [handlePageWheel]);

  return (
    <div className="app-shell flex h-screen w-screen overflow-hidden" style={{ background: "var(--cv-bg)" }}>
      <div className="app-shell-orb app-shell-orb-a" />
      <div className="app-shell-orb app-shell-orb-b" />
      <div className="app-shell-orb app-shell-orb-c" />
      <div className="app-shell-noise" />
      {/* Sidebar */}
      <Sidebar />

      {/* Main Content */}
      <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
        {/* Header */}
        <Header />

        {/* Tab Content */}
        <main ref={mainScrollRef} className="app-main-scroll flex-1 overflow-y-auto overflow-x-hidden p-4 lg:p-6">
          <AnimatePresence mode="wait">
            <motion.div
              key={activeTab}
              initial={{ opacity: 0, x: 16 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: -16 }}
              transition={{ duration: 0.25, ease: [0.25, 0.46, 0.45, 0.94] }}
              className="h-full"
            >
              <ActiveComponent />
            </motion.div>
          </AnimatePresence>
        </main>

        {/* Status Ticker */}
        <StatusTicker />
      </div>
    </div>
  );
}

function StatusTicker() {
  const { statusMessages, serverRunning, vpnConnected, scanning } = useAppStore();
  const latest = statusMessages[statusMessages.length - 1] || "";

  return (
    <div className="status-ticker h-7 flex items-center px-4 gap-4 border-t border-white/5 text-[11px] font-mono">
      <div className="flex items-center gap-2 shrink-0">
        <span className={`status-dot ${serverRunning ? "online" : "offline"}`} />
        <span className="text-cv-subtext">Server</span>
      </div>
      <div className="flex items-center gap-2 shrink-0">
        <span className={`status-dot ${vpnConnected ? "online" : "offline"}`} />
        <span className="text-cv-subtext">VPN</span>
      </div>
      {scanning && (
        <div className="flex items-center gap-2 shrink-0">
          <span className="status-dot pending" />
          <span className="text-cv-subtext">Scanning</span>
        </div>
      )}
      <div className="flex-1 overflow-hidden">
        <div className="truncate text-cv-subtext/60">{latest}</div>
      </div>
      <div className="shrink-0 text-cv-subtext/40">CinaVault Premium Build 129</div>
    </div>
  );
}
