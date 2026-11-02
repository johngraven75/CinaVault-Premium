// CinaVault Premium — App Vault frontend service.
//
// The vault passphrase is the required unlock credential. It is sent to the
// backend only for setup/unlock/change operations, is never stored in
// localStorage, and is never written to a log. A backup file exported from
// the app still requires the same passphrase to restore — it is not a key
// that unlocks the vault on its own.
import { invoke } from "@tauri-apps/api/core";

export type VaultStatus = {
  configured: boolean;
  unlocked: boolean;
  needsSetup: boolean;
  legacyPlaintext: boolean;
  sealedDb: boolean;
};

export function getVaultStatus(): Promise<VaultStatus> {
  return invoke<VaultStatus>("vault_status");
}

export function setupVault(
  passphrase: string,
  confirm: string,
): Promise<VaultStatus> {
  return invoke<VaultStatus>("vault_setup", { passphrase, confirm });
}

export function unlockVault(passphrase: string): Promise<VaultStatus> {
  return invoke<VaultStatus>("vault_unlock", { passphrase });
}

export function lockVault(): Promise<VaultStatus> {
  return invoke<VaultStatus>("vault_lock");
}

export function changeVaultPassphrase(
  oldPassphrase: string,
  newPassphrase: string,
  newConfirm: string,
): Promise<VaultStatus> {
  return invoke<VaultStatus>("vault_change_passphrase", {
    oldPassphrase,
    newPassphrase,
    newConfirm,
  });
}

export function exportVaultBackup(
  passphrase: string,
  exportPath: string,
): Promise<string> {
  return invoke<string>("vault_export_backup", { passphrase, exportPath });
}

export function importVaultBackup(
  passphrase: string,
  backupPath: string,
  overwrite: boolean,
): Promise<VaultStatus> {
  return invoke<VaultStatus>("vault_import_backup", {
    passphrase,
    backupPath,
    overwrite,
  });
}

export function migrateVault(
  passphrase: string,
  confirm: string,
  backupPath: string,
): Promise<VaultStatus> {
  return invoke<VaultStatus>("vault_migrate", {
    passphrase,
    confirm,
    backupPath,
  });
}
