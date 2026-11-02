// CinaVault Premium — App Vault gate screen.
//
// Shown instead of any library tab while the vault needs setup or is locked.
// It never renders library data. After a successful unlock the app reloads,
// because the backend swaps the live database connection to the decrypted
// session database on unlock.
import React, { useState } from "react";
import { setupVault, unlockVault } from "../services/vault";
import type { VaultStatus } from "../services/vault";

export const VAULT_LOSS_WARNING =
  "If you lose your passphrase, your vault cannot be recovered. There is no backdoor, " +
  "no recovery key held by CinaVault, and no plaintext fallback. An optional backup file " +
  "you export yourself still requires your passphrase to restore — it is not a key that " +
  "unlocks the vault on its own.";

export default function VaultLock({
  status,
  onUnlocked,
}: {
  status: VaultStatus;
  onUnlocked: () => void;
}) {
  const [passphrase, setPassphrase] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const isSetup = status.needsSetup;

  async function handleSubmit(event: React.FormEvent) {
    event.preventDefault();
    setError(null);
    setBusy(true);
    try {
      if (isSetup) {
        await setupVault(passphrase, confirm);
      } else {
        await unlockVault(passphrase);
      }
      onUnlocked();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        padding: 32,
        gap: 16,
        maxWidth: 520,
        margin: "0 auto",
        textAlign: "center",
      }}
    >
      <h2>{isSetup ? "Set up your app vault" : "Your app vault is locked"}</h2>
      <p>
        {isSetup
          ? "Choose a passphrase. It is the only credential that unlocks this vault on every launch."
          : "Enter your passphrase to unlock your library. A wrong passphrase keeps the vault locked and exposes no data."}
      </p>
      <p style={{ fontWeight: 600 }}>{VAULT_LOSS_WARNING}</p>
      <form
        onSubmit={handleSubmit}
        style={{ display: "flex", flexDirection: "column", gap: 12, width: "100%" }}
      >
        <input
          type="password"
          value={passphrase}
          onChange={(e) => setPassphrase(e.target.value)}
          placeholder={isSetup ? "Choose a passphrase (at least 8 characters)" : "Passphrase"}
          autoComplete="off"
          required
        />
        {isSetup && (
          <input
            type="password"
            value={confirm}
            onChange={(e) => setConfirm(e.target.value)}
            placeholder="Confirm passphrase"
            autoComplete="off"
            required
          />
        )}
        {error && <div role="alert">{error}</div>}
        <button type="submit" disabled={busy}>
          {busy ? "Working…" : isSetup ? "Set up vault" : "Unlock vault"}
        </button>
      </form>
      <p>
        Existing plaintext libraries are not encrypted by this screen. Use the
        Vault panel in Settings for an explicit, backed-up migration.
      </p>
    </div>
  );
}
