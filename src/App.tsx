// CinaVault Premium — Main Application Shell
import React, { useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
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
import AdvancedTab from "./components/tabs/AdvancedTab";
import CloudNASTab from "./components/tabs/CloudNASTab";
import PluginsTab from "./components/tabs/PluginsTab";
import AIDiagnosticsTab from "./components/tabs/AIDiagnosticsTab";
import SettingsTab from "./components/tabs/SettingsTab";

const TAB_COMPONENTS: Record<TabId, React.FC> = {
  home: HomeTab,
  sources: MediaSourcesTab,
  downloads: DownloadsTab,
  livetv: LiveTVTab,
  server: ServerTab,
  security: SecurityTab,
  advanced: AdvancedTab,
  cloud: CloudNASTab,
  plugins: PluginsTab,
  ai: AIDiagnosticsTab,
  settings: SettingsTab,
};

export default function App() {
  const { activeTab, currentTheme, sidebarCollapsed, setSettings, setTheme, addStatusMessage } = useAppStore();

  // Load settings on mount
  useEffect(() => {
    const init = async () => {
      try {
        const settings = await invoke<Record<string, string>>("get_all_settings");
        setSettings(settings);
        const theme = settings.theme || "prism_fusion";
        setTheme(theme);
        applyTheme(theme);
        addStatusMessage("Settings loaded successfully");
      } catch (e) {
        // Running outside Tauri (dev mode) — use defaults
        applyTheme("prism_fusion");
        addStatusMessage("Running in development mode");
      }
    };
    init();
  }, []);

  // Apply theme when it changes
  useEffect(() => {
    applyTheme(currentTheme);
  }, [currentTheme]);

  const ActiveComponent = TAB_COMPONENTS[activeTab] || HomeTab;

  return (
    <div className="flex h-screen w-screen overflow-hidden" style={{ background: "var(--cv-bg)" }}>
      {/* Sidebar */}
      <Sidebar />

      {/* Main Content */}
      <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
        {/* Header */}
        <Header />

        {/* Tab Content */}
        <main className="flex-1 overflow-y-auto overflow-x-hidden p-4 lg:p-6">
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
    <div className="h-7 flex items-center px-4 gap-4 border-t border-white/5 text-[11px] font-mono"
         style={{ background: "rgba(0,0,0,0.3)" }}>
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
      <div className="shrink-0 text-cv-subtext/40">v1.0.0-beta.1</div>
    </div>
  );
}
