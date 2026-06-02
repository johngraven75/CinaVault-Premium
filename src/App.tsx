// CinaVault Premium — Main Application Shell (with Error Handling)
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
async function saveAllSettingsToBackend(state: Record<string, string>): Promise<void> {
  try {
    const entries = Object.entries(state);
    for (const [key, value] of entries) {
      await invoke("set_setting", { key, value });
    }
  } catch (error) {
    // Dev mode fallback — save to localStorage
    try { 
      localStorage.setItem("cinavault_state", JSON.stringify(state)); 
    } catch (storageError) {
      console.warn("Failed to save state:", storageError);
    }
  }
}

export default function App(): JSX.Element {
  const {
    activeTab, currentTheme, sidebarCollapsed,
    setSettings, setTheme, addStatusMessage,
    getPersistedState, restorePersistedState,
  } = useAppStore();

  const isSaving = useRef<boolean>(false);
  const mainScrollRef = useRef<HTMLElement | null>(null);

  // ── Persist: Save all settings on demand ──
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

  // ── Init: Restore and setup ──
  useEffect(() => {
    (async () => {
      try {
        await restorePersistedState();
        applyTheme(currentTheme);
        pluginEngine.initialize();
        
        // Set window title
        const window = await getCurrentWindow();
        await window.setTitle("CinaVault Premium");
      } catch (error) {
        console.error("Initialization error:", error);
        addStatusMessage("Failed to initialize application");
      }
    })();
  }, [restorePersistedState, currentTheme, addStatusMessage]);

  // ── Auto-save on state change ──
  useEffect(() => {
    const timer = setTimeout(() => { void saveState(); }, 1000);
    return () => clearTimeout(timer);
  }, [activeTab, currentTheme, sidebarCollapsed, saveState]);

  // ── Wheel scroll delegation ──
  const handleWheel = useCallback((e: WheelEvent) => {
    if (!(e.target instanceof Element)) return;
    
    const root = mainScrollRef.current;
    if (!root) return;

    const scrollable = findScrollableAncestor(e.target, root);
    const deltaPixels = getWheelDeltaPixels(e);

    if (!canScrollInDirection(scrollable, deltaPixels)) {
      const parentScroll = findScrollableAncestor(scrollable.parentElement || root, root);
      if (canScrollInDirection(parentScroll, deltaPixels)) {
        const wheelScrolledTop = getWheelScrolledTop(e);
        parentScroll.scrollTop += wheelScrolledTop;
        e.preventDefault();
      }
    }
  }, []);

  const CurrentTabComponent = TAB_COMPONENTS[activeTab];

  return (
    <div className="cv-app min-h-screen flex overflow-hidden" style={{ background: "var(--cv-bg-primary)" }}>
      {/* Sidebar */}
      <Sidebar />

      {/* Main Content */}
      <main className="flex-1 flex flex-col overflow-hidden">
        <Header />
        
        {/* Scrollable Tab Area */}
        <div
          ref={mainScrollRef}
          className="flex-1 overflow-y-auto overflow-x-hidden"
          onWheel={handleWheel}
          style={{ background: "var(--cv-bg-secondary)" }}
        >
          <AnimatePresence mode="wait">
            <motion.div
              key={activeTab}
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -10 }}
              transition={{ duration: 0.15 }}
              className="p-6"
            >
              {CurrentTabComponent ? <CurrentTabComponent /> : <div>Tab not found</div>}
            </motion.div>
          </AnimatePresence>
        </div>
      </main>
    </div>
  );
}
