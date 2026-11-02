// CinaVault Premium — Vault management panel (rendered in Settings).
//
// Lock, change passphrase, export/import a backup, and migrate a legacy
// plaintext library. The backup is sealed material only: restoring it still
// requires the same passphrase, and it never replaces or bypasses it.
import React, { useState } from "react";
import {
  changeVaultPassphrase,
  exportVaultBackup,
  importVaultBackup,
  lockVault,
  migrateVault,
} from "../services/vault";

export default function VaultPanel() {
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [oldPassphrase, setOldPassphrase] = useState("");
  const [newPassphrase, setNewPassphrase] = useState("");
  const [newConfirm, setNewConfirm] = useState("");
  const [backupPath, setBackupPath] = useState("");
  const [migratePath, setMigratePath] = useState("");

  async function run(action: () => Promise<unknown>, ok: string) {
    setMessage(null);
    setError(null);
    try {
      const result = await action();
      setMessage(typeof result === "string" ? result : ok);
      if (ok === "Vault locked.") {
        window.location.reload();
      }
    } catch (err) {
      setError(String(err));
    }
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      <h3>App Vault</h3>
      <p>
        Your library database and generated artwork are encrypted at rest and
        unlocked only with your passphrase. Losing the passphrase is
        unrecoverable unless you kept a backup you exported yourself — and
        that backup still requires the passphrase.
      </p>
      <button
        onClick={() => run(() => lockVault(), "Vault locked.")}
      >
        Lock vault now
      </button>
      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        <input
          type="password"
          value={oldPassphrase}
          onChange={(e) => setOldPassphrase(e.target.value)}
          placeholder="Current passphrase"
          autoComplete="off"
        />
        <input
          type="password"
          value={newPassphrase}
          onChange={(e) => setNewPassphrase(e.target.value)}
          placeholder="New passphrase (at least 8 characters)"
          autoComplete="off"
        />
        <input
          type="password"
          value={newConfirm}
          onChange={(e) => setNewConfirm(e.target.value)}
          placeholder="Confirm new passphrase"
          autoComplete="off"
        />
        <button
          onClick={() =>
            run(
              () => changeVaultPassphrase(oldPassphrase, newPassphrase, newConfirm),
              "Passphrase changed. The next lock seals the vault with the new passphrase.",
            )
          }
        >
          Change passphrase
        </button>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        <input
          value={backupPath}
          onChange={(e) => setBackupPath(e.target.value)}
          placeholder="Backup file path, outside the vault folder"
        />
        <button
          onClick={() =>
            run(
              () => exportVaultBackup(oldPassphrase, backupPath),
              "Backup exported.",
            )
          }
        >
          Export backup (still requires passphrase)
        </button>
        <button
          onClick={() =>
            run(
              () => importVaultBackup(oldPassphrase, backupPath, false),
              "Backup imported. Unlock the vault with your passphrase.",
            )
          }
        >
          Import backup
        </button>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        <input
          value={migratePath}
          onChange={(e) => setMigratePath(e.target.value)}
          placeholder="Sealed backup path, outside the vault folder"
        />
        <button
          onClick={() =>
            run(
              () => migrateVault(oldPassphrase, "MIGRATE", migratePath),
              "Migration complete. Your library is now sealed; the plaintext copy was removed.",
            )
          }
        >
          Migrate plaintext library (explicit, backed up first)
        </button>
      </div>
      {message && <div role="status">{message}</div>}
      {error && <div role="alert">{error}</div>}
    </div>
  );
}
