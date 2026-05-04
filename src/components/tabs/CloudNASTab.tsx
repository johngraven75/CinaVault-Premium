// CinaVault Premium - Cloud & NAS Tab
import React, { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "../../store/appStore";
import { Cloud, HardDrive, Globe, Link, XCircle, LogIn, Server } from "lucide-react";

interface CloudService {
  id: string;
  name: string;
  icon: string;
  status: "connected" | "disconnected" | "placeholder";
  description: string;
}

const BASE_SERVICES: CloudService[] = [
  { id: "wd_mycloud", name: "WD My Cloud", icon: "💾", status: "disconnected", description: "Western Digital NAS access" },
  { id: "synology", name: "Synology QuickConnect", icon: "🗄️", status: "disconnected", description: "Synology NAS via QuickConnect" },
  { id: "remote", name: "Remote Connection", icon: "🌐", status: "disconnected", description: "Custom remote endpoint" },
  { id: "dropbox", name: "Dropbox", icon: "📦", status: "placeholder", description: "Cloud storage integration" },
  { id: "onedrive", name: "OneDrive", icon: "☁️", status: "disconnected", description: "Microsoft cloud storage with username + password" },
  { id: "gdrive", name: "Google Drive", icon: "📁", status: "disconnected", description: "Google Drive connectivity with username + password" },
];

export default function CloudNASTab() {
  const { addStatusMessage, settings, setSetting } = useAppStore();
  const [services, setServices] = useState<CloudService[]>(BASE_SERVICES);
  const [wdLogin, setWdLogin] = useState({ host: "", username: "", password: "" });
  const [synoLogin, setSynoLogin] = useState({ quickconnect_id: "", username: "", password: "" });
  const [remoteEndpoint, setRemoteEndpoint] = useState({ url: "", api_key: "" });
  const [oneDriveLogin, setOneDriveLogin] = useState({ username: "", password: "" });
  const [gDriveLogin, setGDriveLogin] = useState({ username: "", password: "" });
  const [activeConfig, setActiveConfig] = useState<string | null>(null);

  useEffect(() => {
    setOneDriveLogin({
      username: settings.cloud_onedrive_username || "",
      password: settings.cloud_onedrive_password || "",
    });
    setGDriveLogin({
      username: settings.cloud_gdrive_username || "",
      password: settings.cloud_gdrive_password || "",
    });

    setServices((prev) => prev.map((service) => {
      if (service.id === "onedrive" && settings.cloud_onedrive_connected === "true") {
        return { ...service, status: "connected" };
      }
      if (service.id === "gdrive" && settings.cloud_gdrive_connected === "true") {
        return { ...service, status: "connected" };
      }
      return service;
    }));
  }, [settings.cloud_onedrive_connected, settings.cloud_gdrive_connected, settings.cloud_onedrive_username, settings.cloud_onedrive_password, settings.cloud_gdrive_username, settings.cloud_gdrive_password]);

  const persistSetting = async (key: string, value: string) => {
    setSetting(key, value);
    try {
      await invoke("set_setting", { key, value });
    } catch {
      // Ignore dev-mode persistence errors
    }
  };

  const connectService = async (id: string) => {
    setServices((prev) => prev.map((s) => (s.id === id ? { ...s, status: "connected" } : s)));

    if (id === "onedrive") {
      await persistSetting("cloud_onedrive_username", oneDriveLogin.username);
      await persistSetting("cloud_onedrive_password", oneDriveLogin.password);
      await persistSetting("cloud_onedrive_connected", "true");
    }

    if (id === "gdrive") {
      await persistSetting("cloud_gdrive_username", gDriveLogin.username);
      await persistSetting("cloud_gdrive_password", gDriveLogin.password);
      await persistSetting("cloud_gdrive_connected", "true");
    }

    addStatusMessage(`Connected to ${id}`);
    setActiveConfig(null);
  };

  const disconnectService = async (id: string) => {
    setServices((prev) => prev.map((s) => (s.id === id ? { ...s, status: "disconnected" } : s)));

    if (id === "onedrive") await persistSetting("cloud_onedrive_connected", "false");
    if (id === "gdrive") await persistSetting("cloud_gdrive_connected", "false");

    addStatusMessage(`Disconnected from ${id}`);
  };

  return (
    <div className="space-y-5">
      <div className="glass-panel p-5">
        <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
          <Cloud size={16} className="text-cv-accent" /> Cloud & NAS Services
        </h3>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {services.map((service) => (
            <motion.div key={service.id} whileHover={{ scale: 1.01 }} className="glass-panel-2 p-4 rounded-lg">
              <div className="flex items-center justify-between mb-3">
                <div className="flex items-center gap-3">
                  <span className="text-2xl">{service.icon}</span>
                  <div>
                    <div className="text-sm font-semibold">{service.name}</div>
                    <div className="text-[10px] text-cv-subtext">{service.description}</div>
                  </div>
                </div>
                <span className={`status-dot ${service.status === "connected" ? "online" : service.status === "placeholder" ? "pending" : "offline"}`} />
              </div>

              {service.status === "placeholder" ? (
                <div className="text-[10px] text-cv-subtext italic py-2 text-center bg-white/[0.02] rounded">
                  Coming soon - Integration in development
                </div>
              ) : service.status === "connected" ? (
                <button onClick={() => disconnectService(service.id)} className="cv-btn cv-btn-danger text-xs w-full">
                  <XCircle size={12} /> Disconnect
                </button>
              ) : (
                <button onClick={() => setActiveConfig(activeConfig === service.id ? null : service.id)} className="cv-btn cv-btn-primary text-xs w-full">
                  <LogIn size={12} /> Configure
                </button>
              )}
            </motion.div>
          ))}
        </div>
      </div>

      {activeConfig === "wd_mycloud" && (
        <motion.div initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} className="glass-panel p-5">
          <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
            <HardDrive size={16} className="text-cv-accent" /> WD My Cloud Login
          </h3>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
            <div>
              <label className="section-label">Host / IP</label>
              <input value={wdLogin.host} onChange={(e) => setWdLogin({ ...wdLogin, host: e.target.value })} className="cv-input" placeholder="192.168.1.100" />
            </div>
            <div>
              <label className="section-label">Username</label>
              <input value={wdLogin.username} onChange={(e) => setWdLogin({ ...wdLogin, username: e.target.value })} className="cv-input" />
            </div>
            <div>
              <label className="section-label">Password</label>
              <input type="password" value={wdLogin.password} onChange={(e) => setWdLogin({ ...wdLogin, password: e.target.value })} className="cv-input" />
            </div>
          </div>
          <div className="flex gap-2 mt-4">
            <button onClick={() => connectService("wd_mycloud")} className="cv-btn cv-btn-primary">
              <LogIn size={14} /> Connect
            </button>
            <button onClick={() => setActiveConfig(null)} className="cv-btn cv-btn-secondary">Cancel</button>
          </div>
        </motion.div>
      )}

      {activeConfig === "synology" && (
        <motion.div initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} className="glass-panel p-5">
          <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
            <Server size={16} className="text-cv-accent" /> Synology QuickConnect
          </h3>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
            <div>
              <label className="section-label">QuickConnect ID</label>
              <input value={synoLogin.quickconnect_id} onChange={(e) => setSynoLogin({ ...synoLogin, quickconnect_id: e.target.value })} className="cv-input" placeholder="mynas" />
            </div>
            <div>
              <label className="section-label">Username</label>
              <input value={synoLogin.username} onChange={(e) => setSynoLogin({ ...synoLogin, username: e.target.value })} className="cv-input" />
            </div>
            <div>
              <label className="section-label">Password</label>
              <input type="password" value={synoLogin.password} onChange={(e) => setSynoLogin({ ...synoLogin, password: e.target.value })} className="cv-input" />
            </div>
          </div>
          <div className="flex gap-2 mt-4">
            <button onClick={() => connectService("synology")} className="cv-btn cv-btn-primary">
              <LogIn size={14} /> Connect
            </button>
            <button onClick={() => setActiveConfig(null)} className="cv-btn cv-btn-secondary">Cancel</button>
          </div>
        </motion.div>
      )}

      {activeConfig === "remote" && (
        <motion.div initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} className="glass-panel p-5">
          <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
            <Globe size={16} className="text-cv-accent" /> Remote Connection Endpoint
          </h3>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            <div>
              <label className="section-label">Endpoint URL</label>
              <input value={remoteEndpoint.url} onChange={(e) => setRemoteEndpoint({ ...remoteEndpoint, url: e.target.value })} className="cv-input" placeholder="https://my-server.com:8096" />
            </div>
            <div>
              <label className="section-label">API Key (optional)</label>
              <input value={remoteEndpoint.api_key} onChange={(e) => setRemoteEndpoint({ ...remoteEndpoint, api_key: e.target.value })} className="cv-input" />
            </div>
          </div>
          <div className="flex gap-2 mt-4">
            <button onClick={() => connectService("remote")} className="cv-btn cv-btn-primary">
              <Link size={14} /> Connect
            </button>
            <button onClick={() => setActiveConfig(null)} className="cv-btn cv-btn-secondary">Cancel</button>
          </div>
        </motion.div>
      )}

      {activeConfig === "onedrive" && (
        <motion.div initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} className="glass-panel p-5">
          <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
            <Cloud size={16} className="text-cv-accent" /> OneDrive Credentials
          </h3>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            <div>
              <label className="section-label">Username / Email</label>
              <input value={oneDriveLogin.username} onChange={(e) => setOneDriveLogin({ ...oneDriveLogin, username: e.target.value })} className="cv-input" />
            </div>
            <div>
              <label className="section-label">Password</label>
              <input type="password" value={oneDriveLogin.password} onChange={(e) => setOneDriveLogin({ ...oneDriveLogin, password: e.target.value })} className="cv-input" />
            </div>
          </div>
          <div className="flex gap-2 mt-4">
            <button onClick={() => connectService("onedrive")} className="cv-btn cv-btn-primary">
              <LogIn size={14} /> Save + Connect
            </button>
            <button onClick={() => setActiveConfig(null)} className="cv-btn cv-btn-secondary">Cancel</button>
          </div>
        </motion.div>
      )}

      {activeConfig === "gdrive" && (
        <motion.div initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} className="glass-panel p-5">
          <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
            <Cloud size={16} className="text-cv-accent" /> Google Drive Credentials
          </h3>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            <div>
              <label className="section-label">Username / Email</label>
              <input value={gDriveLogin.username} onChange={(e) => setGDriveLogin({ ...gDriveLogin, username: e.target.value })} className="cv-input" />
            </div>
            <div>
              <label className="section-label">Password</label>
              <input type="password" value={gDriveLogin.password} onChange={(e) => setGDriveLogin({ ...gDriveLogin, password: e.target.value })} className="cv-input" />
            </div>
          </div>
          <div className="flex gap-2 mt-4">
            <button onClick={() => connectService("gdrive")} className="cv-btn cv-btn-primary">
              <LogIn size={14} /> Save + Connect
            </button>
            <button onClick={() => setActiveConfig(null)} className="cv-btn cv-btn-secondary">Cancel</button>
          </div>
        </motion.div>
      )}
    </div>
  );
}
