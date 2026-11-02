// Tauri commands for the app vault. These live in a separate file so the
// core cryptography in vault.rs stays free of Tauri types and testable
// without a frontend. The commands are registered in lib.rs.
//
// IMPORTANT: the live database connection is swapped, never duplicated.
// vault_unlock replaces the in-memory locked database with the session
// database; vault_lock checkpoints it, seals it, and replaces it with a
// fresh empty in-memory database. Library commands therefore see no real
// data while locked — they see empty tables, and the frontend gate blocks
// the library UI entirely.

use crate::db::Database;
use crate::vault::{
    change_passphrase as core_change_passphrase, export_backup as core_export_backup,
    import_backup as core_import_backup, is_configured, lock_vault_files,
    migrate_vault as core_migrate, read_metadata, session_db_path, setup_vault as core_setup,
    unlock_vault_files, vault_status as core_status, VaultStatus,
};
use std::path::PathBuf;
use tauri::{Manager, State};

use crate::AppState;

fn app_dir(state: &State<AppState>) -> PathBuf {
    state.app_data_dir.clone()
}

#[tauri::command]
pub fn vault_status(state: State<AppState>) -> Result<VaultStatus, String> {
    let session = state.vault.lock().map_err(|e| e.to_string())?;
    Ok(core_status(&app_dir(&state), &session))
}

#[tauri::command]
pub fn vault_setup(
    state: State<AppState>,
    passphrase: String,
    confirm: String,
) -> Result<VaultStatus, String> {
    let dir = app_dir(&state);
    let mut session = state.vault.lock().map_err(|e| e.to_string())?;
    core_setup(&dir, &mut session, &passphrase, &confirm)?;
    // Setup immediately unlocks the fresh vault into a session database,
    // so first-run setup does not end in a locked, empty app. On the next
    // launch the passphrase is required again.
    let session_path = unlock_vault_files(&dir, &mut session, &passphrase)?;
    drop(session);
    let mut db = state.db.lock().map_err(|e| e.to_string())?;
    *db = Database::new(session_path.to_str().unwrap())
        .map_err(|e| format!("Could not open session database: {e}"))?;
    let session = state.vault.lock().map_err(|e| e.to_string())?;
    Ok(core_status(&dir, &session))
}

#[tauri::command]
pub fn vault_unlock(state: State<AppState>, passphrase: String) -> Result<VaultStatus, String> {
    if !is_configured(&app_dir(&state)) {
        return Err("No vault is set up for this library".into());
    }
    let dir = app_dir(&state);
    let session_path = {
        let mut session = state.vault.lock().map_err(|e| e.to_string())?;
        unlock_vault_files(&dir, &mut session, &passphrase)?
    };
    let mut db = state.db.lock().map_err(|e| e.to_string())?;
    let new_db = Database::new(session_path.to_str().unwrap())
        .map_err(|e| format!("Could not open session database: {e}"))?;
    *db = new_db;
    // Re-run the table setup the startup path runs for a normal launch.
    // Provider initialisation is intentionally not re-run here: it runs on
    // legacy launches, and vault users keep their providers inside the
    // sealed database.
    if let Err(e) = crate::user_data::ensure_tables(&db) {
        log::warn!("Vault unlock: profile tables could not be prepared: {e}");
    }
    if let Err(e) = crate::discovery::ensure_tables(&db) {
        log::warn!("Vault unlock: discovery tables could not be prepared: {e}");
    }
    if let Err(e) = crate::collections::ensure_tables(&db) {
        log::warn!("Vault unlock: collection tables could not be prepared: {e}");
    }
    if let Err(e) = crate::metadata_ext::initialize_metadata_providers(&db) {
        log::warn!("Vault unlock: metadata providers could not be initialised: {e}");
    }
    drop(db);
    let session = state.vault.lock().map_err(|e| e.to_string())?;
    Ok(core_status(&dir, &session))
}

#[tauri::command]
pub fn vault_lock(state: State<AppState>) -> Result<VaultStatus, String> {
    let dir = app_dir(&state);
    // Checkpoint WAL into the session file before sealing it.
    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let _ = db.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
    }
    let mut session = state.vault.lock().map_err(|e| e.to_string())?;
    let was_unlocked = session.unlocked;
    lock_vault_files(&dir, &mut session)?;
    drop(session);
    if was_unlocked {
        let mut db = state.db.lock().map_err(|e| e.to_string())?;
        *db = Database::new(":memory:")
            .map_err(|e| format!("Could not open locked placeholder database: {e}"))?;
    }
    let session = state.vault.lock().map_err(|e| e.to_string())?;
    Ok(core_status(&dir, &session))
}

#[tauri::command]
pub fn vault_change_passphrase(
    state: State<AppState>,
    old_passphrase: String,
    new_passphrase: String,
    new_confirm: String,
) -> Result<VaultStatus, String> {
    let dir = app_dir(&state);
    let mut session = state.vault.lock().map_err(|e| e.to_string())?;
    core_change_passphrase(
        &dir,
        &mut session,
        &old_passphrase,
        &new_passphrase,
        &new_confirm,
    )?;
    Ok(core_status(&dir, &session))
}

#[tauri::command]
pub fn vault_export_backup(
    state: State<AppState>,
    passphrase: String,
    export_path: String,
) -> Result<String, String> {
    let dir = app_dir(&state);
    let session = state.vault.lock().map_err(|e| e.to_string())?;
    let path = PathBuf::from(&export_path);
    core_export_backup(&dir, &session, &passphrase, &path)?;
    Ok(format!(
        "Vault backup written to {}. It still requires your passphrase to restore; \
         the backup file alone cannot unlock anything.",
        path.display()
    ))
}

#[tauri::command]
pub fn vault_import_backup(
    state: State<AppState>,
    passphrase: String,
    backup_path: String,
    overwrite: bool,
) -> Result<VaultStatus, String> {
    let dir = app_dir(&state);
    core_import_backup(&dir, &passphrase, &PathBuf::from(&backup_path), overwrite)?;
    let session = state.vault.lock().map_err(|e| e.to_string())?;
    Ok(core_status(&dir, &session))
}

#[tauri::command]
pub fn vault_migrate(
    state: State<AppState>,
    passphrase: String,
    confirm: String,
    backup_path: String,
) -> Result<VaultStatus, String> {
    let dir = app_dir(&state);
    let mut session = state.vault.lock().map_err(|e| e.to_string())?;
    core_migrate(
        &dir,
        &mut session,
        &passphrase,
        &confirm,
        &PathBuf::from(&backup_path),
    )?;
    // Migration sealed the library; unlock it into a session database now.
    let session_path = unlock_vault_files(&dir, &mut session, &passphrase)?;
    drop(session);
    let mut db = state.db.lock().map_err(|e| e.to_string())?;
    *db = Database::new(session_path.to_str().unwrap())
        .map_err(|e| format!("Could not open migrated session database: {e}"))?;
    let session = state.vault.lock().map_err(|e| e.to_string())?;
    let _ = read_metadata(&dir);
    Ok(core_status(&dir, &session))
}
