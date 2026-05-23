// CinaVault Premium — Security Tab (VPN + Antivirus)
import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion } from "framer-motion";
import { useAppStore } from "../../store/appStore";
import { Shield, Wifi, WifiOff, Globe, Scan, RefreshCw, Download, Loader, MapPin, KeyRound } from "lucide-react";

const VPN_LOCATIONS = [
  "US East", "US West", "US Central", "Canada", "UK",
  "Netherlands", "Germany", "France", "Switzerland", "Hong Kong",
];

export default function SecurityTab() {
  const { vpnConnected, vpnLocation, setVpnStatus, addStatusMessage } = useAppStore();
  const [vpnLoading, setVpnLoading] = useState(false);
  const [selectedLocation, setSelectedLocation] = useState("US East");
  const [avScanning, setAvScanning] = useState(false);
  const [vpnInstalled, setVpnInstalled] = useState<boolean | null>(null);
  const [builtInVpnInstalled, setBuiltInVpnInstalled] = useState<boolean | null>(null);
  const [vpnEndpoint, setVpnEndpoint] = useState("");
  const [vpnPublicKey, setVpnPublicKey] = useState("");
  const [vpnConfig, setVpnConfig] = useState("");
  const [avStatus, setAvStatus] = useState<string>("Checking...");

  useEffect(() => { checkVpnStatus(); checkBuiltInSecurity(); }, []);

  const checkVpnStatus = async () => {
    try {
      const status = await invoke<any>("vpn_status");
      setVpnInstalled(status.installed);
      setVpnStatus(status.connected, vpnLocation);
    } catch {
      setVpnInstalled(false);
    }
  };

  const connectVpn = async () => {
    setVpnLoading(true);
    addStatusMessage(`Connecting VPN to ${selectedLocation}...`);
    try {
      const result = await invoke<any>("vpn_connect", { location: selectedLocation });
      if (result.status === "connected") {
        setVpnStatus(true, selectedLocation);
        addStatusMessage(`VPN connected: ${selectedLocation}`);
      } else {
        addStatusMessage(`VPN connection failed`);
      }
    } catch (e) { addStatusMessage(`VPN error: ${e}`); }
    setVpnLoading(false);
  };

  const disconnectVpn = async () => {
    setVpnLoading(true);
    try {
      await invoke("vpn_disconnect");
      setVpnStatus(false, "");
      addStatusMessage("VPN disconnected");
    } catch (e) { addStatusMessage(`VPN disconnect error: ${e}`); }
    setVpnLoading(false);
  };

  const runScan = async () => {
    setAvScanning(true);
    addStatusMessage("Starting antivirus quick scan...");
    try {
      const result = await invoke<any>("run_antivirus_scan");
      addStatusMessage(`Scan ${result.status}`);
    } catch (e) { addStatusMessage(`Scan failed: ${e}`); }
    setAvScanning(false);
  };

  const updateSignatures = async () => {
    addStatusMessage("Updating antivirus signatures...");
    try {
      const result = await invoke<any>("update_av_signatures");
      addStatusMessage(`Signatures ${result.status}`);
    } catch (e) { addStatusMessage(`Update failed: ${e}`); }
  };

  const installTools = async () => {
    addStatusMessage("Installing security tools...");
    try {
      await invoke("install_security_tools");
      addStatusMessage("Security tools installation initiated");
      checkVpnStatus();
    } catch (e) { addStatusMessage(`Install failed: ${e}`); }
  };

  const checkBuiltInSecurity = async () => {
    try {
      const vpn = await invoke<any>("vpnb_status");
      setBuiltInVpnInstalled(Boolean(vpn.installed));
    } catch {
      setBuiltInVpnInstalled(false);
    }
    try {
      const av = await invoke<any>("avb_status");
      setAvStatus(av.status || (av.installed ? "ready" : "unavailable"));
    } catch {
      setAvStatus("unavailable");
    }
  };

  const generateBuiltInVpnConfig = async () => {
    if (!vpnEndpoint.trim() || !vpnPublicKey.trim()) {
      addStatusMessage("Built-in VPN needs an endpoint and public key");
      return;
    }
    try {
      const result = await invoke<any>("vpnb_generate_test_config", {
        endpoint: vpnEndpoint.trim(),
        publicKey: vpnPublicKey.trim(),
      });
      setVpnConfig(result.config || "");
      addStatusMessage("Built-in WireGuard config generated");
    } catch (e) {
      addStatusMessage(`Built-in VPN config failed: ${e}`);
    }
  };

  const connectBuiltInVpn = async () => {
    if (!vpnConfig.trim()) {
      addStatusMessage("Generate or paste a WireGuard config before connecting");
      return;
    }
    setVpnLoading(true);
    try {
      const result = await invoke<any>("vpnb_connect", { config: vpnConfig });
      addStatusMessage(`Built-in VPN ${result.status}: ${result.message || ""}`);
      await checkBuiltInSecurity();
    } catch (e) {
      addStatusMessage(`Built-in VPN error: ${e}`);
    }
    setVpnLoading(false);
  };

  const disconnectBuiltInVpn = async () => {
    setVpnLoading(true);
    try {
      const result = await invoke<any>("vpnb_disconnect");
      addStatusMessage(`Built-in VPN ${result.status}: ${result.message || ""}`);
      await checkBuiltInSecurity();
    } catch (e) {
      addStatusMessage(`Built-in VPN disconnect error: ${e}`);
    }
    setVpnLoading(false);
  };

  const runBuiltInScan = async () => {
    setAvScanning(true);
    try {
      const result = await invoke<any>("avb_scan_path", { path: "C:\\" });
      addStatusMessage(`Built-in antivirus ${result.status}`);
    } catch (e) {
      addStatusMessage(`Built-in antivirus scan failed: ${e}`);
    }
    setAvScanning(false);
  };

  const updateBuiltInSignatures = async () => {
    try {
      const result = await invoke<any>("avb_update_database");
      addStatusMessage(`Built-in antivirus signatures ${result.status}`);
      await checkBuiltInSecurity();
    } catch (e) {
      addStatusMessage(`Built-in antivirus update failed: ${e}`);
    }
  };

  return (
    <div className="space-y-5">
      {/* VPN Section */}
      <div className="glass-panel p-5">
        <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
          <Shield size={16} className="text-cv-accent" /> VPN — Windscribe
        </h3>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-5">
          {/* VPN Status */}
          <div className="glass-panel-2 p-5 rounded-lg text-center">
            <motion.div
              animate={{ scale: vpnConnected ? [1, 1.1, 1] : 1 }}
              transition={{ duration: 2, repeat: vpnConnected ? Infinity : 0 }}
              className={`w-20 h-20 rounded-full mx-auto mb-4 flex items-center justify-center ${
                vpnConnected ? "bg-green-500/20 ring-2 ring-green-500/40" : "bg-cv-danger/20 ring-2 ring-cv-danger/40"
              }`}
            >
              {vpnConnected ? (
                <Wifi size={36} className="text-green-500" />
              ) : (
                <WifiOff size={36} className="text-cv-danger" />
              )}
            </motion.div>
            <div className="text-lg font-bold mb-1">
              {vpnConnected ? "Connected" : "Disconnected"}
            </div>
            {vpnConnected && vpnLocation && (
              <div className="text-sm text-cv-accent flex items-center justify-center gap-1">
                <MapPin size={12} /> {vpnLocation}
              </div>
            )}
            <div className="text-[10px] text-cv-subtext mt-1">
              {vpnInstalled === false ? "Windscribe not installed" : "Windscribe CLI"}
            </div>

            <div className="flex gap-2 mt-4 justify-center">
              {!vpnConnected ? (
                <button onClick={connectVpn} disabled={vpnLoading} className="cv-btn cv-btn-primary">
                  {vpnLoading ? <Loader size={14} className="animate-spin" /> : <Wifi size={14} />}
                  Connect
                </button>
              ) : (
                <button onClick={disconnectVpn} disabled={vpnLoading} className="cv-btn cv-btn-danger">
                  {vpnLoading ? <Loader size={14} className="animate-spin" /> : <WifiOff size={14} />}
                  Disconnect
                </button>
              )}
            </div>
          </div>

          {/* Location Picker */}
          <div className="glass-panel-2 p-5 rounded-lg">
            <label className="section-label mb-3 block">Select Location</label>
            <div className="grid grid-cols-2 gap-2">
              {VPN_LOCATIONS.map(loc => (
                <button
                  key={loc}
                  onClick={() => setSelectedLocation(loc)}
                  className={`px-3 py-2 rounded-lg text-xs font-medium text-left flex items-center gap-2 transition-all ${
                    selectedLocation === loc
                      ? "bg-cv-accent/15 text-cv-accent border border-cv-accent/20"
                      : "bg-white/[0.03] text-cv-subtext hover:bg-white/[0.06] border border-transparent"
                  }`}
                >
                  <Globe size={12} />
                  {loc}
                </button>
              ))}
            </div>
          </div>
        </div>
      </div>

      {/* Built-in VPN Section */}
      <div className="glass-panel p-5">
        <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
          <KeyRound size={16} className="text-cv-accent" /> Built-in VPN — WireGuard
        </h3>
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <div className="space-y-3">
            <div className="text-xs text-cv-subtext">
              Status: {builtInVpnInstalled === false ? "WireGuard not detected" : "WireGuard command surface ready"}
            </div>
            <input value={vpnEndpoint} onChange={e => setVpnEndpoint(e.target.value)} className="cv-input" placeholder="Endpoint host:port" />
            <input value={vpnPublicKey} onChange={e => setVpnPublicKey(e.target.value)} className="cv-input" placeholder="Peer public key" />
            <div className="flex flex-wrap gap-2">
              <button onClick={generateBuiltInVpnConfig} className="cv-btn cv-btn-secondary">
                <KeyRound size={14} /> Generate Config
              </button>
              <button onClick={connectBuiltInVpn} disabled={vpnLoading} className="cv-btn cv-btn-primary">
                {vpnLoading ? <Loader size={14} className="animate-spin" /> : <Wifi size={14} />} Connect Built-in
              </button>
              <button onClick={disconnectBuiltInVpn} disabled={vpnLoading} className="cv-btn cv-btn-danger">
                <WifiOff size={14} /> Disconnect Built-in
              </button>
            </div>
          </div>
          <textarea
            value={vpnConfig}
            onChange={e => setVpnConfig(e.target.value)}
            className="cv-input min-h-[170px] font-mono text-[11px]"
            placeholder="Paste a full WireGuard config here, or generate one from endpoint/public key."
          />
        </div>
      </div>

      {/* Antivirus Section */}
      <div className="glass-panel p-5">
        <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
          <Scan size={16} className="text-cv-accent" /> Antivirus — Windows Defender
        </h3>
        <div className="flex flex-wrap gap-3">
          <button onClick={runScan} disabled={avScanning} className="cv-btn cv-btn-primary">
            {avScanning ? <Loader size={14} className="animate-spin" /> : <Scan size={14} />}
            {avScanning ? "Scanning..." : "Quick Scan"}
          </button>
          <button onClick={updateSignatures} className="cv-btn cv-btn-secondary">
            <RefreshCw size={14} /> Update Signatures
          </button>
          <button onClick={installTools} className="cv-btn cv-btn-secondary">
            <Download size={14} /> Install Security Tools
          </button>
        </div>
      </div>

      {/* Built-in Antivirus Section */}
      <div className="glass-panel p-5">
        <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
          <Shield size={16} className="text-cv-accent" /> Built-in Antivirus — Microsoft Defender
        </h3>
        <div className="text-xs text-cv-subtext mb-3">Status: {avStatus}</div>
        <div className="flex flex-wrap gap-3">
          <button onClick={runBuiltInScan} disabled={avScanning} className="cv-btn cv-btn-primary">
            {avScanning ? <Loader size={14} className="animate-spin" /> : <Scan size={14} />}
            Scan System Drive
          </button>
          <button onClick={updateBuiltInSignatures} className="cv-btn cv-btn-secondary">
            <RefreshCw size={14} /> Update Built-in Signatures
          </button>
        </div>
      </div>
    </div>
  );
}
