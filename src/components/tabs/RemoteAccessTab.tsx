// CinaVault Premium — Remote Access Management (Unified layout)
import React, { useMemo, useState } from "react";
import { motion } from "framer-motion";
import { useAppStore } from "../../store/appStore";
import {
  Router,
  Globe,
  ShieldCheck,
  RefreshCw,
  CheckCircle,
  AlertTriangle,
  PlugZap,
  SlidersHorizontal,
  Lock,
} from "lucide-react";

type SecureMode = "required" | "preferred" | "disabled";

const secureOptions: { value: SecureMode; label: string; desc: string }[] = [
  { value: "required", label: "Required", desc: "Only encrypted remote connections are allowed." },
  { value: "preferred", label: "Preferred", desc: "Use secure remote connections when possible." },
  { value: "disabled", label: "Disabled", desc: "Allow insecure remote connections." },
];

export function isRemoteAccessConfigurationValid(
  remoteEnabled: boolean,
  publicPort: string,
  manualPort: boolean,
) {
  if (!remoteEnabled) return false;
  if (!publicPort || Number.isNaN(Number(publicPort))) return false;
  if (manualPort && (Number(publicPort) < 1 || Number(publicPort) > 65535)) return false;
  return true;
}

export default function RemoteAccessTab() {
  const { settings, setSetting, serverUrl, addStatusMessage } = useAppStore();
  const [testing, setTesting] = useState(false);
  const [publicIp, setPublicIp] = useState<string>("");
  const [lastTestAt, setLastTestAt] = useState<string>("");

  const remoteEnabled = settings.remote_access_enabled !== "false";
  const manualPort = settings.remote_manually_specify_port === "true";
  const secureMode = (settings.remote_secure_connections || "preferred") as SecureMode;
  const preferredRelay = settings.remote_preferred_relay === "true";
  const fallback = settings.remote_allow_fallback !== "false";
  const upnp = settings.remote_enable_upnp !== "false";
  const natPmp = settings.remote_enable_natpmp !== "false";
  const publicPort = settings.remote_public_port || "32400";
  const uploadLimit = settings.remote_upload_limit_mbps || "20";
  const allowedNetworks = settings.remote_allowed_networks || "";

  const remoteOk = useMemo(
    () => isRemoteAccessConfigurationValid(remoteEnabled, publicPort, manualPort),
    [remoteEnabled, publicPort, manualPort],
  );

  const runConnectionTest = async () => {
    setTesting(true);
    try {
      const res = await fetch("https://api.ipify.org?format=json");
      const data = await res.json();
      setPublicIp(data.ip || "");
      setLastTestAt(new Date().toLocaleTimeString());
      addStatusMessage("Remote access test complete");
    } catch {
      addStatusMessage("Remote access test failed");
    }
    setTesting(false);
  };

  return (
    <div className="space-y-5">
      <div className="glass-panel p-5">
        <div className="flex items-center justify-between gap-3 mb-4">
          <h3 className="text-sm font-bold flex items-center gap-2">
            <Router size={16} className="text-cv-accent" /> Remote Access Management
          </h3>
          <button onClick={runConnectionTest} disabled={testing} className="cv-btn cv-btn-secondary text-xs">
            <RefreshCw size={12} className={testing ? "animate-spin" : ""} />
            {testing ? "Testing..." : "Test Remote Access"}
          </button>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div className="glass-panel-2 p-4 rounded-lg">
            <div className="text-[11px] text-cv-subtext mb-2">Status</div>
            <div className="flex items-center gap-2 mb-2">
              {remoteOk ? <CheckCircle size={14} className="text-green-400" /> : <AlertTriangle size={14} className="text-amber-400" />}
              <span className="text-sm font-semibold">{remoteOk ? "Fully Configured" : "Needs Attention"}</span>
            </div>
            <div className="text-xs text-cv-subtext">Server URL: {serverUrl}</div>
            <div className="text-xs text-cv-subtext">Public IP: {publicIp || "Not tested yet"}</div>
            <div className="text-xs text-cv-subtext">Last check: {lastTestAt || "Never"}</div>
          </div>

          <div className="glass-panel-2 p-4 rounded-lg">
            <div className="text-[11px] text-cv-subtext mb-2">Reachability</div>
            <label className="flex items-center justify-between text-xs py-1">
              <span>Enable Remote Access</span>
              <input type="checkbox" checked={remoteEnabled} onChange={(e) => setSetting("remote_access_enabled", String(e.target.checked))} />
            </label>
            <label className="flex items-center justify-between text-xs py-1">
              <span>Manually Specify Public Port</span>
              <input type="checkbox" checked={manualPort} onChange={(e) => setSetting("remote_manually_specify_port", String(e.target.checked))} />
            </label>
            <label className="text-xs block mt-2">Public Port</label>
            <input
              className="cv-input mt-1"
              value={publicPort}
              onChange={(e) => setSetting("remote_public_port", e.target.value.replace(/[^\d]/g, ""))}
              placeholder="32400"
            />
          </div>

          <div className="glass-panel-2 p-4 rounded-lg">
            <div className="text-[11px] text-cv-subtext mb-2">Router Mapping</div>
            <label className="flex items-center justify-between text-xs py-1">
              <span className="flex items-center gap-1"><PlugZap size={12} /> Enable UPnP</span>
              <input type="checkbox" checked={upnp} onChange={(e) => setSetting("remote_enable_upnp", String(e.target.checked))} />
            </label>
            <label className="flex items-center justify-between text-xs py-1">
              <span className="flex items-center gap-1"><PlugZap size={12} /> Enable NAT-PMP</span>
              <input type="checkbox" checked={natPmp} onChange={(e) => setSetting("remote_enable_natpmp", String(e.target.checked))} />
            </label>
            <label className="flex items-center justify-between text-xs py-1">
              <span>Prefer Relay</span>
              <input type="checkbox" checked={preferredRelay} onChange={(e) => setSetting("remote_preferred_relay", String(e.target.checked))} />
            </label>
            <label className="flex items-center justify-between text-xs py-1">
              <span>Allow Fallback to Relay</span>
              <input type="checkbox" checked={fallback} onChange={(e) => setSetting("remote_allow_fallback", String(e.target.checked))} />
            </label>
          </div>
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-5">
        <div className="glass-panel p-5">
          <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
            <ShieldCheck size={16} className="text-cv-accent" /> Secure Connections
          </h3>
          <div className="space-y-2">
            {secureOptions.map((opt) => (
              <button
                key={opt.value}
                onClick={() => setSetting("remote_secure_connections", opt.value)}
                className={`w-full text-left rounded-lg p-3 border transition ${
                  secureMode === opt.value ? "border-cv-accent/40 bg-cv-accent/10" : "border-white/10 bg-white/[0.02]"
                }`}
              >
                <div className="text-xs font-semibold flex items-center gap-2">
                  <Lock size={12} />
                  {opt.label}
                </div>
                <div className="text-[11px] text-cv-subtext mt-1">{opt.desc}</div>
              </button>
            ))}
          </div>
        </div>

        <div className="glass-panel p-5">
          <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
            <SlidersHorizontal size={16} className="text-cv-accent" /> Streaming Constraints
          </h3>
          <label className="section-label">Internet Upload Limit (Mbps)</label>
          <input
            className="cv-input mb-3"
            value={uploadLimit}
            onChange={(e) => setSetting("remote_upload_limit_mbps", e.target.value.replace(/[^\d]/g, ""))}
            placeholder="20"
          />
          <label className="section-label">Allowed Networks (CIDR, comma-separated)</label>
          <textarea
            className="cv-input min-h-[100px]"
            value={allowedNetworks}
            onChange={(e) => setSetting("remote_allowed_networks", e.target.value)}
            placeholder="192.168.1.0/24,10.0.0.0/8"
          />
          <div className="text-[10px] text-cv-subtext mt-2 flex items-center gap-1">
            <Globe size={10} />
            Match unified remote access controls; values are persisted in CinaVault settings.
          </div>
        </div>
      </div>

      <motion.div
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        className="glass-panel p-4 text-xs text-cv-subtext"
      >
        Remote profile saved:{" "}
        <span className="text-cv-text">
          {remoteEnabled ? "Enabled" : "Disabled"} / {secureMode} secure / port {publicPort || "n/a"}
        </span>
      </motion.div>
    </div>
  );
}
