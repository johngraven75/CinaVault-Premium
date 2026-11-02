// CinaVault Premium — Security Tab (bundled WireGuard + Windows Defender)
import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { motion } from "framer-motion";
import { useAppStore } from "../../store/appStore";
import {
  Shield,
  Wifi,
  WifiOff,
  Scan,
  RefreshCw,
  Download,
  Loader,
  FileKey,
  CheckCircle,
  AlertTriangle,
} from "lucide-react";
import TabBanner from "../experience/TabBanner";

type VpnProfile = {
  name: string;
  active: boolean;
  addresses: string[];
  endpoint: string;
  allowed_ips: string[];
  verified: boolean;
  is_default: boolean;
};

type VpnInstallResult = {
  status: string;
  alreadyInstalled?: boolean;
  version?: string;
  msiUrl?: string;
  msiSha256?: string;
  sha256Verified?: boolean;
  sha256Pinned?: boolean;
  signatureVerified?: boolean;
  engine?: string;
  engineSource?: string;
  rebootRequired?: boolean;
  message?: string;
};

type VpnStatus = {
  installed: boolean;
  engineBundled?: boolean;
  engineReadiness?: { source?: string };
  connected: boolean;
  activeProfile?: string | null;
  profiles: VpnProfile[];
  details?: string;
  startupStatus?: { status: string; error?: string };
};

export default function SecurityTab() {
  const { vpnConnected, vpnLocation, setVpnStatus, addStatusMessage } = useAppStore();
  const [vpnLoading, setVpnLoading] = useState(false);
  const [avScanning, setAvScanning] = useState(false);
  const [vpnInstalled, setVpnInstalled] = useState<boolean | null>(null);
  const [vpnEngineBundled, setVpnEngineBundled] = useState(false);
  const [installingWireGuard, setInstallingWireGuard] = useState(false);
  const [installDetails, setInstallDetails] = useState("");
  const [profiles, setProfiles] = useState<VpnProfile[]>([]);
  const [selectedProfile, setSelectedProfile] = useState("");
  const [vpnDetails, setVpnDetails] = useState("");
  const [vpnError, setVpnError] = useState("");
  const [autoConnect, setAutoConnect] = useState(false);

  useEffect(() => {
    void checkVpnStatus();
  }, []);

  const checkVpnStatus = async () => {
    try {
      const status = await invoke<VpnStatus>("vpn_status");
      setVpnInstalled(status.installed);
      setVpnEngineBundled(
        status.engineBundled === true ||
          status.engineReadiness?.source === "bundled",
      );
      setProfiles(status.profiles ?? []);
      setVpnDetails(status.details ?? "");
      if (status.startupStatus?.status === "failed") {
        setVpnError(status.startupStatus.error ?? "WireGuard startup auto-connect failed safely");
      }
      const active = status.activeProfile ?? status.profiles?.find((profile) => profile.active)?.name ?? "";
      setSelectedProfile((current) => current || active || status.profiles?.[0]?.name || "");
      setVpnStatus(status.connected, active);
    } catch (error) {
      setVpnInstalled(false);
      setVpnDetails(String(error));
      setVpnError(String(error));
    }
  };

  const importProfile = async () => {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "WireGuard profile", extensions: ["conf"] }],
    });
    if (!selected || Array.isArray(selected)) return;

    setVpnLoading(true);
    try {
      const profile = await invoke<VpnProfile>("vpn_import_profile", {
        sourcePath: selected,
      });
      setSelectedProfile(profile.name);
      addStatusMessage(`WireGuard profile stored permanently: ${profile.name}`);
      await checkVpnStatus();
    } catch (error) {
      setVpnError(String(error));
      addStatusMessage(`WireGuard profile import failed: ${error}`);
    } finally {
      setVpnLoading(false);
    }
  };

  const connectVpn = async () => {
    if (!selectedProfile) {
      addStatusMessage("Import and select a WireGuard profile before connecting");
      return;
    }
    setVpnLoading(true);
    addStatusMessage(`Connecting WireGuard profile ${selectedProfile}...`);
    try {
      const result = await invoke<{ status: string }>("vpn_connect", {
        profile: selectedProfile,
      });
      if (result.status === "connected") {
        setVpnError("");
        setVpnStatus(true, selectedProfile);
        addStatusMessage(`VPN connected: ${selectedProfile}`);
      } else {
        addStatusMessage(`VPN connection state: ${result.status}`);
      }
      await checkVpnStatus();
    } catch (error) {
      setVpnError(String(error));
      addStatusMessage(`VPN error: ${error}`);
    } finally {
      setVpnLoading(false);
    }
  };

  const selectDefault = async () => {
    try {
      await invoke("vpn_select_default", { profile: selectedProfile, autoConnect });
      setVpnError("");
      addStatusMessage(`Default WireGuard profile saved: ${selectedProfile}`);
      await checkVpnStatus();
    } catch (error) {
      setVpnError(String(error));
    }
  };

  // Maps the backend's `WIREGUARD_<NAME> (<code>): <message>` errors to
  // the one-line explanation shown in the UI. Codes 10/11/12 are connect
  // steps (tunnel service / running / handshake) and can only surface if
  // an install-then-connect flow fails at connect; 20-23 and 1223 are the
  // install steps themselves. The raw backend string is kept as a suffix
  // so no diagnostic detail is hidden.
  const wireguardInstallErrorMessage = (error: unknown): string => {
    const raw = String(error);
    const match = raw.match(/\(\d+\)/);
    const code = match ? Number(match[1]) : null;
    const prefix =
      code === 20
        ? "Download failed — check your connection and try again."
        : code === 21
          ? "Signature or hash check failed — the installer was NOT installed. Retry from the official download only."
          : code === 22
            ? "The WireGuard installer failed — try again, or install the MSI from wireguard.com manually and retry."
            : code === 23
              ? "Installer finished but no official client was found under Program Files — this does not count as installed. A bundled-only engine is not an installed client."
              : code === 1223
                ? "Administrator approval was declined — Windows requires it to install WireGuard. CinaVault still works; retry when ready."
                : code === 10
                  ? "WireGuard could not create the tunnel service — check the profile and that no other VPN uses the same tunnel."
                  : code === 11
                    ? "WireGuard tunnel did not start; the tunnel service was removed."
                    : code === 12
                      ? "WireGuard tunnel started but the server never answered the handshake — check the endpoint, keys and UDP traffic."
                      : null;
    return prefix ? `${prefix} (${raw})` : raw;
  };

  const installWireGuard = async (force = false) => {
    setInstallingWireGuard(true);
    setVpnError("");
    setInstallDetails(
      "Downloading and verifying the official WireGuard client… Administrator approval will be requested; do not close CinaVault.",
    );
    try {
      const result = await invoke<VpnInstallResult>("vpn_install_wireguard", { force });
      if (result.status === "already_installed") {
        setInstallDetails(
          "Official WireGuard client is already installed and verified; no download or reinstall was performed.",
        );
        addStatusMessage("WireGuard already installed");
      } else {
        const shaNote =
          result.sha256Verified
            ? `SHA-256 verified (${result.msiSha256 ?? "unknown"})`
            : result.sha256Pinned
              ? "SHA-256 check failed to verify"
              : `SHA-256 of the downloaded MSI (no official hash published to compare against): ${result.msiSha256 ?? "unknown"}`;
        setInstallDetails(
          `Official WireGuard ${result.version ?? ""} installed and verified under Program Files. ${shaNote}${result.rebootRequired ? " A reboot is required to finish the installation." : ""}`,
        );
        addStatusMessage(`WireGuard installed: ${result.engine ?? "Program Files"}`);
      }
      await checkVpnStatus();
    } catch (error) {
      const message = wireguardInstallErrorMessage(error);
      setVpnError(message);
      setInstallDetails("");
      addStatusMessage(`WireGuard install failed: ${message}`);
    } finally {
      setInstallingWireGuard(false);
    }
  };

  const disconnectVpn = async () => {
    setVpnLoading(true);
    try {
      await invoke("vpn_disconnect");
      setVpnStatus(false, "");
      addStatusMessage("VPN disconnected");
      await checkVpnStatus();
    } catch (error) {
      addStatusMessage(`VPN disconnect error: ${error}`);
    } finally {
      setVpnLoading(false);
    }
  };

  const runScan = async () => {
    setAvScanning(true);
    addStatusMessage("Starting antivirus quick scan...");
    try {
      const result = await invoke<{ status: string }>("run_antivirus_scan");
      addStatusMessage(`Scan ${result.status}`);
    } catch (error) {
      addStatusMessage(`Scan failed: ${error}`);
    } finally {
      setAvScanning(false);
    }
  };

  const updateSignatures = async () => {
    addStatusMessage("Updating antivirus signatures...");
    try {
      const result = await invoke<{ status: string }>("update_av_signatures");
      addStatusMessage(`Signatures ${result.status}`);
    } catch (error) {
      addStatusMessage(`Update failed: ${error}`);
    }
  };

  return (
    <div className="space-y-5">
      <TabBanner icon={Shield} eyebrow="Trusted Compute" title="Security Matrix" subtitle="Manage identity, encryption, VPN protection, threat scanning, and privacy boundaries." accent="from-emerald-300/28 to-teal-500/10" accentText="text-emerald-100" />
      <div className="glass-panel p-5">
        <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
          <Shield size={16} className="text-cv-accent" /> VPN — Official WireGuard
        </h3>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-5">
          <div className="glass-panel-2 p-5 rounded-lg text-center">
            <motion.div
              animate={{ scale: vpnConnected ? [1, 1.1, 1] : 1 }}
              transition={{ duration: 2, repeat: vpnConnected ? Infinity : 0 }}
              className={`w-20 h-20 rounded-full mx-auto mb-4 flex items-center justify-center ${
                vpnConnected
                  ? "bg-green-500/20 ring-2 ring-green-500/40"
                  : "bg-cv-danger/20 ring-2 ring-cv-danger/40"
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
              <div className="text-sm text-cv-accent">{vpnLocation}</div>
            )}
            <div className="text-[10px] text-cv-subtext mt-1 flex items-center justify-center gap-1">
              {vpnInstalled && !vpnEngineBundled ? <CheckCircle size={11} /> : <AlertTriangle size={11} />}
              {vpnInstalled && !vpnEngineBundled
                ? "Official client installed and verified"
                : vpnInstalled
                  ? "Bundled engine only — official client not installed"
                  : "Official engine unavailable"}
            </div>
            {vpnDetails && <div className="text-[10px] text-cv-subtext mt-2">{vpnDetails}</div>}
            {vpnError && <div role="alert" className="mt-3 rounded border border-red-400/30 bg-red-400/10 p-2 text-xs text-red-200">{vpnError}</div>}

            <div className="flex gap-2 mt-4 justify-center">
              {!vpnConnected ? (
                <button
                  onClick={connectVpn}
                  disabled={vpnLoading || !vpnInstalled || !selectedProfile}
                  className="cv-btn cv-btn-primary"
                >
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
            {(!vpnInstalled || vpnEngineBundled) && (
              <button
                onClick={() => void installWireGuard(false)}
                disabled={installingWireGuard || vpnLoading}
                className="cv-btn cv-btn-secondary w-full"
              >
                {installingWireGuard ? <Loader size={14} className="animate-spin" /> : <Download size={14} />}
                {installingWireGuard
                  ? "Installing official WireGuard…"
                  : vpnEngineBundled
                    ? "Install official WireGuard client (bundled engine only)"
                    : "Install official WireGuard client"}
              </button>
            )}
            {installDetails && <div className="text-[10px] text-cv-subtext mt-2">{installDetails}</div>}
            <label className="flex items-center gap-2 text-xs">
              <input type="checkbox" checked={autoConnect} onChange={(event) => setAutoConnect(event.target.checked)} />
              Connect this verified profile when CinaVault starts
            </label>
            <button onClick={selectDefault} disabled={!profiles.find((profile) => profile.name === selectedProfile)?.verified} className="cv-btn cv-btn-secondary w-full">
              Save as verified default
            </button>
          </div>

          <div className="glass-panel-2 p-5 rounded-lg space-y-4">
            <div>
              <label className="section-label mb-2 block">Permanent profile</label>
              <select
                value={selectedProfile}
                onChange={(event) => setSelectedProfile(event.target.value)}
                className="cv-input w-full"
                disabled={vpnConnected}
              >
                <option value="">Select a stored profile</option>
                {profiles.map((profile) => (
                  <option key={profile.name} value={profile.name}>
                    {profile.name}{profile.active ? " — active" : ""}
                  </option>
                ))}
              </select>
            </div>
            <button onClick={importProfile} disabled={vpnLoading || vpnConnected} className="cv-btn cv-btn-secondary w-full">
              <FileKey size={14} /> Import .conf profile
            </button>
            <p className="text-xs text-cv-subtext leading-relaxed">
              Imported profiles are copied into CinaVault application data and retained across restarts and upgrades. Private keys are never committed to the repository or embedded in public installers.
            </p>
          </div>
        </div>
      </div>

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
          <button onClick={importProfile} disabled={vpnLoading || vpnConnected} className="cv-btn cv-btn-secondary">
            <Download size={14} /> Add VPN Profile
          </button>
        </div>
      </div>
    </div>
  );
}
