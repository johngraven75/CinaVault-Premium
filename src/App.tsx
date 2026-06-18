// CinaVault Premium — Main Application Shell
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

async function saveAllSettingsToBackend(state: Record<string, string>): Promise<void> {
  try {
    for (const [key, value] of Object.entries(state)) {
      await invoke("set_setting", { key, value });
    }
  } catch (error) {
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
  const mainScrollRef = useRef<HTMLElement | null>(null);

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
        await restorePersistedState();
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

  const handleWheel = useCallback((event: React.WheelEvent<HTMLDivElement>): void => {
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
    <div className="cv-app min-h-screen flex overflow-hidden" style={{ background: "var(--cv-bg-primary)" }}>
      <Sidebar />

      <main className="flex-1 flex flex-col overflow-hidden">
        <Header />

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
