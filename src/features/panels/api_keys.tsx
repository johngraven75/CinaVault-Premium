// API keys for apps: issue (shown once), list by prefix, revoke.
import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Copy, KeyRound, Trash2 } from "lucide-react";

import {
  API_KEY_PERMISSIONS,
  apiKeyStatus,
  permissionLabels,
  relativeTime,
  type ApiKeySummary,
} from "../serverAdminLogic.ts";
import type { FeaturePanel } from "./types";

const ApiKeysPanel: FeaturePanel = ({ enabled }) => {
  const [keys, setKeys] = useState<ApiKeySummary[]>([]);
  const [name, setName] = useState("");
  const [permissions, setPermissions] = useState<string[]>(["library:read", "stream:play"]);
  const [revealed, setRevealed] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const reload = useCallback(async () => {
    try {
      setKeys(await invoke<ApiKeySummary[]>("api_keys_list"));
    } catch (error) {
      setMessage(String(error));
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  const issue = async () => {
    try {
      const issued = await invoke<{ key: string; summary: ApiKeySummary }>("api_key_issue", {
        name: name.trim(),
        permissions,
      });
      setRevealed(issued.key);
      setName("");
      setMessage(null);
      await reload();
    } catch (error) {
      setMessage(String(error));
    }
  };

  const revoke = async (key: ApiKeySummary) => {
    if (!window.confirm(`Revoke ${key.name}? Apps using it stop working immediately.`)) return;
    try {
      await invoke("api_key_revoke", { id: key.id });
      await reload();
    } catch (error) {
      setMessage(String(error));
    }
  };

  return (
    <div className="space-y-2">
      <p className="text-[11px] text-cv-subtext">
        Apps send <code>X-Api-Key: &lt;key&gt;</code> (or <code>Authorization: ApiKey &lt;key&gt;</code>) to the media server.
        {!enabled && " Keys are refused while this switch is off."}
      </p>
      <form
        className="flex flex-wrap items-center gap-1.5"
        onSubmit={(event) => {
          event.preventDefault();
          void issue();
        }}
      >
        <input
          className="cv-input min-w-[9rem] flex-1 text-xs"
          placeholder="App name, e.g. Living room TV"
          maxLength={60}
          value={name}
          onChange={(event) => setName(event.target.value)}
        />
        {API_KEY_PERMISSIONS.map((permission) => (
          <label key={permission.value} className="flex items-center gap-1 text-xs">
            <input
              type="checkbox"
              checked={permissions.includes(permission.value)}
              onChange={(event) =>
                setPermissions((list) =>
                  event.target.checked ? [...list, permission.value] : list.filter((value) => value !== permission.value),
                )
              }
            />
            {permission.label}
          </label>
        ))}
        <button type="submit" className="cv-btn text-xs" disabled={!name.trim() || permissions.length === 0}>
          <KeyRound size={12} /> Issue key
        </button>
      </form>
      {revealed && (
        <div className="rounded-lg border border-emerald-300/25 bg-emerald-300/[0.06] p-2 text-[11px]">
          <div className="mb-1 text-emerald-100">Copy this key now. It will not be shown again.</div>
          <div className="flex items-center gap-1.5">
            <code className="flex-1 break-all font-mono text-xs">{revealed}</code>
            <button
              type="button"
              className="cv-btn cv-btn-secondary px-2 py-1 text-xs"
              aria-label="Copy key"
              onClick={() => void navigator.clipboard?.writeText(revealed).then(() => setMessage("Key copied"))}
            >
              <Copy size={12} />
            </button>
            <button type="button" className="cv-btn cv-btn-secondary px-2 py-1 text-xs" onClick={() => setRevealed(null)}>
              Done
            </button>
          </div>
        </div>
      )}
      <ul className="space-y-0.5">
        {keys.length === 0 && <li className="text-[11px] text-cv-subtext">No keys issued.</li>}
        {keys.map((key) => (
          <li key={key.id} className={`flex items-center gap-2 rounded px-1.5 py-1 ${apiKeyStatus(key) === "revoked" ? "opacity-50" : ""}`}>
            <code className="font-mono text-[11px] text-cv-accent">{key.prefix}…</code>
            <span className="flex-1 truncate text-xs">{key.name}</span>
            <span className="text-[10px] text-cv-subtext">{permissionLabels(key.permissions)}</span>
            <span className="w-20 text-right text-[10px] text-cv-subtext">
              {apiKeyStatus(key) === "revoked" ? "revoked" : key.lastUsedAt ? `used ${relativeTime(key.lastUsedAt)}` : "never used"}
            </span>
            {apiKeyStatus(key) === "active" && (
              <button type="button" className="cv-btn cv-btn-secondary px-2 py-1 text-xs" aria-label={`Revoke ${key.name}`} onClick={() => void revoke(key)}>
                <Trash2 size={12} />
              </button>
            )}
          </li>
        ))}
      </ul>
      {message && <div className="text-[11px] text-cv-subtext" role="status">{message}</div>}
    </div>
  );
};

export default ApiKeysPanel;
