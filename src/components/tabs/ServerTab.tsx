// CinaVault Premium — Server Tab (Jellyfin/Emby Management)
import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion } from "framer-motion";
import { useAppStore } from "../../store/appStore";
import {
  Server, Play, Square, ExternalLink, RefreshCw, CheckCircle, XCircle,
  LayoutDashboard, Library, Users, Puzzle, ListTodo, FileText, Monitor, Smartphone, Import
} from "lucide-react";

const ADMIN_PAGES = [
  { id: "dashboard", label: "Dashboard", icon: LayoutDashboard },
  { id: "libraries", label: "Libraries", icon: Library },
  { id: "users", label: "Users", icon: Users },
  { id: "plugins", label: "Plugins", icon: Puzzle },
  { id: "tasks", label: "Tasks", icon: ListTodo },
  { id: "logs", label: "Logs", icon: FileText },
  { id: "sessions", label: "Sessions API", icon: Monitor },
  { id: "devices", label: "Devices API", icon: Smartphone },
];

export default function ServerTab() {
  const { serverRunning, serverType, serverUrl, setServerStatus, addStatusMessage } = useAppStore();
  const [serverInfo, setServerInfo] = useState<any>(null);
  const [embyCompat, setEmbyCompat] = useState<any>(null);
  const [customUrl, setCustomUrl] = useState("http://localhost:8096");
  const [apiKey, setApiKey] = useState("");
  const [checking, setChecking] = useState(false);

  useEffect(() => { checkServer(); }, []);

  const checkServer = async () => {
    setChecking(true);
    try {
      const status = await invoke<any>("get_server_status", { serverType, baseUrl: customUrl });
      setServerStatus(status.running, serverType, customUrl);
      if (status.running) {
        setServerInfo(status);
        addStatusMessage(`Server detected: ${status.server_name} v${status.version}`);
      }
    } catch {
      setServerStatus(false, serverType, customUrl);
    }
    setChecking(false);
  };

  const startServer = async () => {
    addStatusMessage(`Starting ${serverType} server...`);
    try {
      const result = await invoke<any>("start_server", { serverType });
      addStatusMessage(`Server ${result.status}: ${result.path}`);
      setTimeout(checkServer, 3000);
    } catch (e) { addStatusMessage(`Start failed: ${e}`); }
  };

  const stopServer = async () => {
    try {
      await invoke("stop_server", { serverType });
      addStatusMessage(`${serverType} server stopped`);
      setServerStatus(false, serverType, serverUrl);
      setServerInfo(null);
    } catch (e) { addStatusMessage(`Stop failed: ${e}`); }
  };

  const openAdmin = async (page: string) => {
    try {
      await invoke("open_admin_page", { baseUrl: customUrl, page });
    } catch (e) { addStatusMessage(`Failed to open: ${e}`); }
  };

  const importLibraries = async () => {
    if (!apiKey) { addStatusMessage("API key required for import"); return; }
    try {
      const result = await invoke<any>("import_libraries", { baseUrl: customUrl, apiKey });
      addStatusMessage(`Imported ${result.sources_imported} library sources from ${result.libraries_found} libraries`);
    } catch (e) { addStatusMessage(`Import failed: ${e}`); }
  };

  const checkEmbyCompat = async () => {
    try {
      const result = await invoke<any>("check_emby_compat", { baseUrl: customUrl });
      setEmbyCompat(result);
      addStatusMessage(result.compatible ? `Compatible: ${result.product} v${result.version}` : "Compatibility check failed");
    } catch (e) { addStatusMessage(`Check failed: ${e}`); }
  };

  return (
    <div className="space-y-5">
      {/* Server Status */}
      <div className="glass-panel p-5">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-sm font-bold flex items-center gap-2">
            <Server size={16} className="text-cv-accent" /> Server Management
          </h3>
          <div className="flex items-center gap-3">
            <select value={serverType} onChange={e => setServerStatus(serverRunning, e.target.value, serverUrl)} className="cv-select text-xs py-1.5">
              <option value="jellyfin">Jellyfin</option>
              <option value="emby">Emby</option>
            </select>
            <button onClick={checkServer} className="cv-btn cv-btn-secondary text-xs py-1.5">
              <RefreshCw size={12} className={checking ? "animate-spin" : ""} /> Check
            </button>
          </div>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          {/* Status Card */}
          <div className="glass-panel-2 p-4 rounded-lg">
            <div className="flex items-center gap-3 mb-3">
              <div className={`w-12 h-12 rounded-xl flex items-center justify-center ${serverRunning ? "bg-green-500/20" : "bg-cv-danger/20"}`}>
                <Server size={24} className={serverRunning ? "text-green-500" : "text-cv-danger"} />
              </div>
              <div>
                <div className="text-sm font-bold">{serverRunning ? "Running" : "Stopped"}</div>
                <div className="text-[10px] text-cv-subtext capitalize">{serverType} Server</div>
              </div>
            </div>
            {serverInfo && (
              <div className="space-y-1 text-xs text-cv-subtext">
                <div>Name: <span className="text-cv-text">{serverInfo.server_name}</span></div>
                <div>Version: <span className="text-cv-text">{serverInfo.version}</span></div>
              </div>
            )}
            <div className="flex gap-2 mt-4">
              {!serverRunning ? (
                <button onClick={startServer} className="cv-btn cv-btn-primary text-xs flex-1">
                  <Play size={12} /> Start Server
                </button>
              ) : (
                <button onClick={stopServer} className="cv-btn cv-btn-danger text-xs flex-1">
                  <Square size={12} /> Stop Server
                </button>
              )}
            </div>
          </div>

          {/* Connection Config */}
          <div className="glass-panel-2 p-4 rounded-lg">
            <label className="section-label">Server URL</label>
            <input value={customUrl} onChange={e => setCustomUrl(e.target.value)} className="cv-input mb-2" />
            <label className="section-label">API Key (for imports)</label>
            <input type="password" value={apiKey} onChange={e => setApiKey(e.target.value)} className="cv-input mb-3" placeholder="Enter API key" />
            <div className="flex gap-2">
              <button onClick={importLibraries} className="cv-btn cv-btn-secondary text-xs flex-1">
                <Import size={12} /> Import Libraries
              </button>
            </div>
          </div>

          {/* Emby Compatibility */}
          <div className="glass-panel-2 p-4 rounded-lg">
            <label className="section-label">Emby SDK Compatibility</label>
            <button onClick={checkEmbyCompat} className="cv-btn cv-btn-secondary text-xs mb-3 w-full">
              <CheckCircle size={12} /> Check Compatibility
            </button>
            {embyCompat && (
              <div className="space-y-1 text-xs">
                <div className="flex items-center gap-2">
                  {embyCompat.compatible ? <CheckCircle size={12} className="text-green-500" /> : <XCircle size={12} className="text-cv-danger" />}
                  <span>{embyCompat.compatible ? "Compatible" : "Not Compatible"}</span>
                </div>
                {embyCompat.product && <div className="text-cv-subtext">Product: {embyCompat.product}</div>}
                {embyCompat.version && <div className="text-cv-subtext">Version: {embyCompat.version}</div>}
                <div className="text-cv-subtext">Emby API: {embyCompat.emby_api ? "Yes" : "No"}</div>
                <div className="text-cv-subtext">Jellyfin API: {embyCompat.jellyfin_api ? "Yes" : "No"}</div>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Admin Console */}
      <div className="glass-panel p-5">
        <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
          <LayoutDashboard size={16} className="text-cv-accent" /> Admin Console
        </h3>
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
          {ADMIN_PAGES.map(page => (
            <motion.button
              key={page.id}
              whileHover={{ scale: 1.02 }}
              whileTap={{ scale: 0.98 }}
              onClick={() => openAdmin(page.id)}
              className="glass-panel-2 p-4 rounded-lg flex flex-col items-center gap-2 hover:bg-white/5 transition-colors"
            >
              <page.icon size={24} className="text-cv-accent" />
              <span className="text-xs font-semibold">{page.label}</span>
              <ExternalLink size={10} className="text-cv-subtext" />
            </motion.button>
          ))}
        </div>
      </div>
    </div>
  );
}
