// CinaVault Premium — Remote Access Management (Unified layout)
import React, { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
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
  UserPlus,
  LogIn,
  KeyRound,
  Copy,
  RotateCw,
  Power,
  BookOpen,
  ServerCog,
  BadgeCheck,
} from "lucide-react";

type SecureMode = "required" | "preferred" | "disabled";

type RemoteAccessUser = {
  id: number;
  email: string;
  display_name?: string | null;
  access_key_preview: string;
  enabled: boolean;
  permissions: string[];
  created_at: string;
  updated_at: string;
  last_login?: string | null;
};

type RemoteProvision = RemoteAccessUser & {
  access_key: string;
};

type RemotePrincipal = {
  id: number;
  email: string;
  display_name?: string | null;
  auth_method: "password" | "access_key";
  session_token: string;
  expires_at: string;
  permissions: string[];
};

type RemoteKeyRotation = {
  email: string;
  access_key: string;
  access_key_preview: string;
};

const secureOptions: { value: SecureMode; label: string; desc: string }[] = [
  { value: "required", label: "Required", desc: "Only encrypted remote connections are allowed." },
  { value: "preferred", label: "Preferred", desc: "Use secure remote connections when possible." },
  { value: "disabled", label: "Disabled", desc: "Allow insecure remote connections." },
];

const connectionSteps = [
  {
    title: "Server address",
    body: "Enter the CinaVault media server URL exactly as issued, including http or https and the port when one is provided.",
  },
  {
    title: "User ID and password",
    body: "Use the assigned user ID or email with the password from the server owner. Password login proves the account is active.",
  },
  {
    title: "Access token",
    body: "Paste the issued token or access key when a client asks for API key, token, or key access. Tokens can be rotated by the server owner.",
  },
  {
    title: "Confirm access",
    body: "Run an access check, then open the remote library. If login works but media does not load, verify the server URL and public port.",
  },
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
  const [accounts, setAccounts] = useState<RemoteAccessUser[]>([]);
  const [accountForm, setAccountForm] = useState({ displayName: "", email: "", password: "" });
  const [passwordLogin, setPasswordLogin] = useState({ email: "", password: "" });
  const [keyLogin, setKeyLogin] = useState("");
  const [latestKey, setLatestKey] = useState<RemoteKeyRotation | RemoteProvision | null>(null);
  const [lastPrincipal, setLastPrincipal] = useState<RemotePrincipal | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

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

  useEffect(() => {
    loadAccounts();
  }, []);

  const loadAccounts = async () => {
    try {
      const rows = await invoke<RemoteAccessUser[]>("list_remote_access_users");
      setAccounts(rows);
    } catch (error) {
      addStatusMessage(`Remote accounts unavailable: ${error}`);
    }
  };

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

  const createAccount = async () => {
    setBusy("create");
    try {
      const provision = await invoke<RemoteProvision>("create_remote_access_user", {
        email: accountForm.email,
        password: accountForm.password,
        displayName: accountForm.displayName || null,
      });
      setLatestKey(provision);
      setAccountForm({ displayName: "", email: "", password: "" });
      addStatusMessage(`Remote account ready: ${provision.email}`);
      await loadAccounts();
    } catch (error) {
      addStatusMessage(`Remote account setup failed: ${error}`);
    } finally {
      setBusy(null);
    }
  };

  const loginWithPassword = async () => {
    setBusy("password");
    try {
      const principal = await invoke<RemotePrincipal | null>("authenticate_remote_password", passwordLogin);
      setLastPrincipal(principal);
      addStatusMessage(principal ? `Password access accepted for ${principal.email}` : "Password access denied");
      await loadAccounts();
    } catch (error) {
      addStatusMessage(`Password access failed: ${error}`);
    } finally {
      setBusy(null);
    }
  };

  const loginWithKey = async () => {
    setBusy("key");
    try {
      const principal = await invoke<RemotePrincipal | null>("authenticate_remote_access_key", {
        accessKey: keyLogin,
      });
      setLastPrincipal(principal);
      addStatusMessage(principal ? `Access key accepted for ${principal.email}` : "Access key denied");
      await loadAccounts();
    } catch (error) {
      addStatusMessage(`Access key check failed: ${error}`);
    } finally {
      setBusy(null);
    }
  };

  const rotateKey = async (email: string) => {
    setBusy(`rotate:${email}`);
    try {
      const rotated = await invoke<RemoteKeyRotation | null>("rotate_remote_access_key", { email });
      if (rotated) {
        setLatestKey(rotated);
        addStatusMessage(`Access key rotated for ${email}`);
      }
      await loadAccounts();
    } catch (error) {
      addStatusMessage(`Key rotation failed: ${error}`);
    } finally {
      setBusy(null);
    }
  };

  const toggleAccount = async (account: RemoteAccessUser) => {
    setBusy(`toggle:${account.email}`);
    try {
      await invoke("set_remote_access_user_enabled", {
        email: account.email,
        enabled: !account.enabled,
      });
      addStatusMessage(`${account.email} ${account.enabled ? "disabled" : "enabled"}`);
      await loadAccounts();
    } catch (error) {
      addStatusMessage(`Remote account update failed: ${error}`);
    } finally {
      setBusy(null);
    }
  };

  const copyLatestKey = async () => {
    if (!latestKey?.access_key) return;
    await navigator.clipboard?.writeText(latestKey.access_key);
    addStatusMessage("Remote access key copied");
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
            <div className="text-xs text-cv-subtext">Remote users: {accounts.length}</div>
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

      <div className="grid grid-cols-1 xl:grid-cols-[1.1fr_0.9fr] gap-5">
        <div className="glass-panel p-5">
          <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
            <UserPlus size={16} className="text-cv-accent" /> Remote Account
          </h3>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
            <div>
              <label className="section-label">Display Name</label>
              <input className="cv-input" value={accountForm.displayName} onChange={(e) => setAccountForm({ ...accountForm, displayName: e.target.value })} />
            </div>
            <div>
              <label className="section-label">Email</label>
              <input className="cv-input" value={accountForm.email} onChange={(e) => setAccountForm({ ...accountForm, email: e.target.value })} />
            </div>
            <div>
              <label className="section-label">Password</label>
              <input type="password" className="cv-input" value={accountForm.password} onChange={(e) => setAccountForm({ ...accountForm, password: e.target.value })} />
            </div>
          </div>
          <div className="mt-3 flex flex-wrap gap-2">
            <button onClick={createAccount} disabled={busy === "create"} className="cv-btn cv-btn-primary text-xs">
              <UserPlus size={12} /> {busy === "create" ? "Saving..." : "Save Account"}
            </button>
            {latestKey?.access_key && (
              <button onClick={copyLatestKey} className="cv-btn cv-btn-secondary text-xs">
                <Copy size={12} /> Copy New Access Key
              </button>
            )}
          </div>
          {latestKey?.access_key && (
            <div className="mt-4 rounded-lg border border-cv-accent/30 bg-cv-accent/10 p-3">
              <div className="text-[11px] text-cv-subtext mb-1">New access key for {latestKey.email}</div>
              <code className="block text-xs text-cv-text break-all">{latestKey.access_key}</code>
            </div>
          )}
        </div>

        <div className="glass-panel p-5">
          <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
            <LogIn size={16} className="text-cv-accent" /> Access Check
          </h3>
          <div className="space-y-3">
            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
              <input className="cv-input" placeholder="email" value={passwordLogin.email} onChange={(e) => setPasswordLogin({ ...passwordLogin, email: e.target.value })} />
              <input type="password" className="cv-input" placeholder="password" value={passwordLogin.password} onChange={(e) => setPasswordLogin({ ...passwordLogin, password: e.target.value })} />
            </div>
            <button onClick={loginWithPassword} disabled={busy === "password"} className="cv-btn cv-btn-secondary text-xs w-full justify-center">
              <LogIn size={12} /> {busy === "password" ? "Checking..." : "Check Email Password"}
            </button>
            <div className="flex gap-2">
              <input className="cv-input flex-1" placeholder="cvra_..." value={keyLogin} onChange={(e) => setKeyLogin(e.target.value)} />
              <button onClick={loginWithKey} disabled={busy === "key"} className="cv-btn cv-btn-secondary text-xs">
                <KeyRound size={12} /> Key
              </button>
            </div>
            <div className="text-xs text-cv-subtext">
              Last accepted: {lastPrincipal ? `${lastPrincipal.email} via ${lastPrincipal.auth_method}` : "None"}
            </div>
          </div>
        </div>
      </div>

      <div className="glass-panel p-5">
        <div className="flex items-center justify-between gap-3 mb-4">
          <h3 className="text-sm font-bold flex items-center gap-2">
            <BookOpen size={16} className="text-cv-accent" /> How to Connect to a Remote Server
          </h3>
          <div className="text-[11px] text-cv-subtext flex items-center gap-1">
            <ServerCog size={12} />
            ID, password, and token access
          </div>
        </div>
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-3">
          {connectionSteps.map((step, index) => (
            <div key={step.title} className="rounded-lg border border-white/10 bg-white/[0.02] p-4 min-h-[148px]">
              <div className="flex items-center gap-2 mb-2">
                <div className="w-7 h-7 rounded-lg bg-cv-accent/15 text-cv-accent flex items-center justify-center text-xs font-bold">
                  {index + 1}
                </div>
                <div className="text-sm font-semibold">{step.title}</div>
              </div>
              <p className="text-xs text-cv-subtext leading-relaxed">{step.body}</p>
            </div>
          ))}
        </div>
        <div className="mt-4 rounded-lg border border-green-500/20 bg-green-500/10 p-3 text-xs text-cv-subtext flex items-start gap-2">
          <BadgeCheck size={14} className="text-green-400 mt-0.5 shrink-0" />
          <span>
            Keep all three issued values together: server URL, user ID/password, and token. The token is for trusted clients and should be replaced if it is shared by mistake.
          </span>
        </div>
      </div>

      <div className="glass-panel p-5">
        <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
          <ShieldCheck size={16} className="text-cv-accent" /> Authorized Remote Users
        </h3>
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-3">
          {accounts.map((account) => (
            <div key={account.id} className="rounded-lg border border-white/10 bg-white/[0.02] p-3">
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
                  <div className="text-sm font-semibold truncate">{account.display_name || account.email}</div>
                  <div className="text-xs text-cv-subtext truncate">{account.email}</div>
                  <div className="text-[11px] text-cv-subtext">Key: ...{account.access_key_preview}</div>
                  <div className="text-[11px] text-cv-subtext">Last login: {account.last_login || "Never"}</div>
                </div>
                <div className={`text-[11px] px-2 py-1 rounded ${account.enabled ? "bg-green-500/15 text-green-300" : "bg-cv-danger/15 text-cv-danger"}`}>
                  {account.enabled ? "Enabled" : "Disabled"}
                </div>
              </div>
              <div className="mt-3 flex flex-wrap gap-2">
                <button onClick={() => rotateKey(account.email)} disabled={busy === `rotate:${account.email}`} className="cv-btn cv-btn-secondary text-xs">
                  <RotateCw size={12} /> Rotate Key
                </button>
                <button onClick={() => toggleAccount(account)} disabled={busy === `toggle:${account.email}`} className="cv-btn cv-btn-secondary text-xs">
                  <Power size={12} /> {account.enabled ? "Disable" : "Enable"}
                </button>
              </div>
            </div>
          ))}
          {accounts.length === 0 && (
            <div className="rounded-lg border border-white/10 bg-white/[0.02] p-4 text-xs text-cv-subtext">
              No remote users saved.
            </div>
          )}
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
