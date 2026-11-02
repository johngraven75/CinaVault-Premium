// CinaVault Premium — App Vault encryption.
//
// Scope: app vault ONLY. This module encrypts CinaVault's own library
// database (cinavault.db) and the generated artwork under the app data
// directory. It does NOT encrypt a whole drive or arbitrary user files.
//
// Passphrase-first design (user clarification, 2026-11-01):
//   * The required unlock credential is a user-selected passphrase.
//   * The passphrase is never stored. It is fed to Argon2id with a random
//     salt; only the salt and an AEAD verification tag are stored.
//   * A system-generated key file is NEVER the required or sole method and
//     NEVER bypasses the passphrase. The optional backup file exported by
//     `vault_export_backup` contains only sealed vault material (salt,
//     verification tag, sealed database). Restoring it still requires the
//     same passphrase; a backup file alone cannot unlock anything.
//   * Losing the passphrase is unrecoverable by design. There is no
//     backdoor, no recovery key held by the app, and no plaintext fallback.
//     The setup UI must say this plainly, and the PR body repeats it.
//
// Cryptography (standard, reviewed primitives only):
//   * AEAD: XChaCha20-Poly1305, random 24-byte nonce per seal, with an
//     authenticated associated-data string that names what is sealed.
//   * KDF: Argon2id, m=19456 KiB, t=2, p=1, 32-byte output.
//   * Keys live only in memory for the session (VaultSession) and are
//     zeroized on lock. They are never written to the database, a source
//     file, a log, or a test fixture, and never handed to the OS keyring
//     (the keyring is for long-lived service credentials, not a vault key
//     that must be re-entered on every launch).
//
// At-rest layout (all inside the app data directory, except backups):
//   * vault.json            — version, KDF parameters, salt, verification
//                             tag (AEAD of a fixed constant). Not secret,
//                             but useless without the passphrase.
//   * cinavault.db.vault    — sealed database bytes.
//   * artwork/**/*.vault    — sealed artwork files (plaintext artwork is
//                             removed while locked).
//   * .vault-session/       — decrypted working copy while unlocked.
//                             Wiped (zero-overwritten, then deleted) on lock.
//   * cinavault.db          — plaintext database. Present ONLY in legacy
//                             (never set up) mode, or while unlocked via the
//                             session copy. Never present while locked.
//
// Startup model (see lib.rs):
//   * First run (no vault.json, no cinavault.db): the app starts with an
//     empty in-memory database and the frontend shows the vault setup
//     gate. No library access happens until setup + unlock.
//   * Vault configured: the app starts locked (in-memory database, no real
//     data) and the frontend shows the vault unlock gate.
//   * Legacy (plaintext cinavault.db exists, no vault.json): the app opens
//     the library normally so existing users are NOT locked out. Vault
//     setup is offered from settings, and migration is an explicit,
//     user-confirmed step (vault_migrate) that first writes a sealed
//     backup outside the vault directory. Nothing is silently encrypted
//     in place.

use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use chacha20poly1305::{
    aead::{Aead, Payload},
    KeyInit, XChaCha20Poly1305, XNonce,
};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use zeroize::{Zeroize, Zeroizing};

pub const VAULT_FILE: &str = "vault.json";
pub const PLAINTEXT_DB: &str = "cinavault.db";
pub const SEALED_DB: &str = "cinavault.db.vault";
pub const SESSION_DIR: &str = ".vault-session";
pub const SESSION_DB: &str = "cinavault.db";
pub const ARTWORK_DIR: &str = "artwork";
pub const SEALED_SUFFIX: &str = ".vault";
pub const MIGRATE_CONFIRM: &str = "MIGRATE";

const MAGIC: &[u8; 4] = b"CVV1";
const VERIFICATION_PLAINTEXT: &[u8] = b"cinavault-vault-verification-v1";
const VERIFICATION_AAD: &[u8] = b"cinavault-vault-verification-v1";
const DB_AAD: &[u8] = b"cinavault-vault-db-v1";
const FILE_AAD: &[u8] = b"cinavault-vault-file-v1";
const MIN_PASSPHRASE_LEN: usize = 8;
const KDF_M_COST: u32 = 19456;
const KDF_T_COST: u32 = 2;
const KDF_P_COST: u32 = 1;

/// In-memory vault session. The key is zeroized when cleared or dropped.
pub struct VaultSession {
    pub unlocked: bool,
    pub key: Option<Zeroizing<[u8; 32]>>,
}

impl Default for VaultSession {
    fn default() -> Self {
        Self {
            unlocked: false,
            key: None,
        }
    }
}

impl VaultSession {
    pub fn clear(&mut self) {
        if let Some(mut key) = self.key.take() {
            key.zeroize();
        }
        self.unlocked = false;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultMetadata {
    pub version: u32,
    pub salt_b64: String,
    pub verification_nonce_b64: String,
    pub verification_ciphertext_b64: String,
    pub kdf_m_cost: u32,
    pub kdf_t_cost: u32,
    pub kdf_p_cost: u32,
    pub sealed_db: bool,
    pub migrated_from_plaintext: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    pub configured: bool,
    pub unlocked: bool,
    /// True only on a true first run: no vault and no existing library.
    pub needs_setup: bool,
    /// True for an existing plaintext library that has never set up a vault.
    pub legacy_plaintext: bool,
    pub sealed_db: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BackupFile {
    version: u32,
    salt_b64: String,
    verification_nonce_b64: String,
    verification_ciphertext_b64: String,
    kdf_m_cost: u32,
    kdf_t_cost: u32,
    kdf_p_cost: u32,
    sealed_db_nonce_b64: String,
    sealed_db_ciphertext_b64: String,
}

pub fn vault_path(app_dir: &Path) -> PathBuf {
    app_dir.join(VAULT_FILE)
}

pub fn sealed_db_path(app_dir: &Path) -> PathBuf {
    app_dir.join(SEALED_DB)
}

pub fn plaintext_db_path(app_dir: &Path) -> PathBuf {
    app_dir.join(PLAINTEXT_DB)
}

pub fn session_dir(app_dir: &Path) -> PathBuf {
    app_dir.join(SESSION_DIR)
}

pub fn session_db_path(app_dir: &Path) -> PathBuf {
    session_dir(app_dir).join(SESSION_DB)
}

pub fn is_configured(app_dir: &Path) -> bool {
    vault_path(app_dir).is_file()
}

pub fn read_metadata(app_dir: &Path) -> Result<VaultMetadata, String> {
    let raw = fs::read_to_string(vault_path(app_dir))
        .map_err(|e| format!("Vault metadata is unreadable: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("Vault metadata is corrupt: {e}"))
}

fn write_metadata_atomic(app_dir: &Path, metadata: &VaultMetadata) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(metadata)
        .map_err(|e| format!("Could not serialize vault metadata: {e}"))?;
    let tmp = app_dir.join(format!("{VAULT_FILE}.tmp"));
    fs::write(&tmp, raw).map_err(|e| format!("Could not write vault metadata: {e}"))?;
    fs::rename(&tmp, vault_path(app_dir))
        .map_err(|e| format!("Could not install vault metadata: {e}"))
}

pub fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    let params = Params::new(KDF_M_COST, KDF_T_COST, KDF_P_COST, Some(32))
        .map_err(|e| format!("Invalid Argon2id parameters: {e}"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; 32];
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|e| format!("Passphrase derivation failed: {e}"))?;
    Ok(key)
}

fn random_bytes<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    OsRng.fill_bytes(&mut buf);
    buf
}

/// Seal bytes: MAGIC || nonce(24) || AEAD ciphertext. Authentication failure
/// on open means the key, the data, or the AAD is wrong — never partial data.
pub fn seal_bytes(key: &[u8; 32], plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>, String> {
    let cipher =
        XChaCha20Poly1305::new_from_slice(key).map_err(|e| format!("Invalid vault key: {e}"))?;
    let nonce_bytes = random_bytes::<24>();
    let nonce = XNonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|e| format!("Vault sealing failed: {e}"))?;
    let mut out = Vec::with_capacity(4 + 24 + ciphertext.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

pub fn open_bytes(key: &[u8; 32], sealed: &[u8], aad: &[u8]) -> Result<Vec<u8>, String> {
    if sealed.len() < 4 + 24 + 16 || &sealed[..4] != MAGIC {
        return Err("Vault data is missing, truncated, or not a CinaVault vault file".into());
    }
    let cipher =
        XChaCha20Poly1305::new_from_slice(key).map_err(|e| format!("Invalid vault key: {e}"))?;
    let nonce = XNonce::from_slice(&sealed[4..28]);
    cipher
        .decrypt(
            nonce,
            Payload {
                msg: &sealed[28..],
                aad,
            },
        )
        .map_err(|_| {
            "Vault authentication failed: wrong passphrase, or the vault data was tampered with"
                .to_string()
        })
}

fn seal_file(key: &[u8; 32], source: &Path, dest: &Path, aad: &[u8]) -> Result<(), String> {
    let plaintext =
        fs::read(source).map_err(|e| format!("Could not read {}: {e}", source.display()))?;
    let sealed = seal_bytes(key, &plaintext, aad)?;
    write_atomic(dest, &sealed)
}

fn write_atomic(dest: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
    }
    let tmp = dest.with_extension("tmp-vault");
    fs::write(&tmp, bytes).map_err(|e| format!("Could not write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, dest).map_err(|e| format!("Could not install {}: {e}", dest.display()))
}

/// Overwrite a file with zeros (same length) and then delete it, so a
/// decrypted working copy does not linger on disk after lock.
pub fn wipe_file(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Ok(());
    }
    let len = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if len > 0 {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .open(path)
            .map_err(|e| format!("Could not wipe {}: {e}", path.display()))?;
        use std::io::{Seek, SeekFrom, Write};
        file.seek(SeekFrom::Start(0))
            .map_err(|e| format!("Could not wipe {}: {e}", path.display()))?;
        let zeros = vec![0u8; 8192];
        let mut remaining = len;
        while remaining > 0 {
            let chunk = std::cmp::min(remaining, zeros.len() as u64) as usize;
            file.write_all(&zeros[..chunk])
                .map_err(|e| format!("Could not wipe {}: {e}", path.display()))?;
            remaining -= chunk as u64;
        }
        let _ = file.sync_all();
    }
    fs::remove_file(path).map_err(|e| format!("Could not delete {}: {e}", path.display()))
}

fn check_passphrase(passphrase: &str, confirm: Option<&str>) -> Result<(), String> {
    if passphrase.chars().count() < MIN_PASSPHRASE_LEN {
        return Err(format!(
            "Choose a passphrase of at least {MIN_PASSPHRASE_LEN} characters"
        ));
    }
    if let Some(expected) = confirm {
        if passphrase != expected {
            return Err("Passphrases do not match".into());
        }
    }
    Ok(())
}

fn make_verification(key: &[u8; 32]) -> Result<(String, String), String> {
    let sealed = seal_bytes(key, VERIFICATION_PLAINTEXT, VERIFICATION_AAD)?;
    Ok((B64.encode(&sealed[4..28]), B64.encode(&sealed[28..])))
}

fn verify_key(metadata: &VaultMetadata, key: &[u8; 32]) -> Result<(), String> {
    let nonce = B64
        .decode(&metadata.verification_nonce_b64)
        .map_err(|_| "Vault verification data is corrupt".to_string())?;
    let ciphertext = B64
        .decode(&metadata.verification_ciphertext_b64)
        .map_err(|_| "Vault verification data is corrupt".to_string())?;
    let mut sealed = Vec::with_capacity(4 + nonce.len() + ciphertext.len());
    sealed.extend_from_slice(MAGIC);
    sealed.extend_from_slice(&nonce);
    sealed.extend_from_slice(&ciphertext);
    let plaintext = open_bytes(key, &sealed, VERIFICATION_AAD)?;
    if plaintext != VERIFICATION_PLAINTEXT {
        return Err("Vault authentication failed: wrong passphrase".into());
    }
    Ok(())
}

fn key_from_passphrase(metadata: &VaultMetadata, passphrase: &str) -> Result<[u8; 32], String> {
    let salt = B64
        .decode(&metadata.salt_b64)
        .map_err(|_| "Vault salt is corrupt".to_string())?;
    let key = derive_key(passphrase, &salt)?;
    verify_key(metadata, &key)?;
    Ok(key)
}

pub fn vault_status(app_dir: &Path, session: &VaultSession) -> VaultStatus {
    let configured = is_configured(app_dir);
    let plaintext_exists = plaintext_db_path(app_dir).is_file();
    let sealed = sealed_db_path(app_dir).is_file();
    VaultStatus {
        configured,
        unlocked: configured && session.unlocked && session.key.is_some(),
        needs_setup: !configured && !plaintext_exists,
        legacy_plaintext: !configured && plaintext_exists,
        sealed_db: sealed,
    }
}

/// Create a fresh vault. Refuses if a vault already exists, and refuses if a
/// plaintext library already exists (use vault_migrate instead, so nothing
/// is silently encrypted in place). On success the session holds the new
/// key; the caller must then create/unlock the session database.
pub fn setup_vault(
    app_dir: &Path,
    session: &mut VaultSession,
    passphrase: &str,
    confirm: &str,
) -> Result<VaultMetadata, String> {
    if is_configured(app_dir) {
        return Err("A vault is already set up for this library".into());
    }
    if plaintext_db_path(app_dir).is_file() {
        return Err(
            "A plaintext library already exists. Set up a vault with an explicit \
             migration (vault_migrate) instead, so your existing library is backed up first."
                .into(),
        );
    }
    check_passphrase(passphrase, Some(confirm))?;
    let salt = random_bytes::<32>();
    let key = derive_key(passphrase, &salt)?;
    let (nonce_b64, ciphertext_b64) = make_verification(&key)?;
    let metadata = VaultMetadata {
        version: 1,
        salt_b64: B64.encode(salt),
        verification_nonce_b64: nonce_b64,
        verification_ciphertext_b64: ciphertext_b64,
        kdf_m_cost: KDF_M_COST,
        kdf_t_cost: KDF_T_COST,
        kdf_p_cost: KDF_P_COST,
        sealed_db: false,
        migrated_from_plaintext: false,
    };
    write_metadata_atomic(app_dir, &metadata)?;
    session.key = Some(Zeroizing::new(key));
    session.unlocked = false;
    Ok(metadata)
}

/// Decrypt the sealed database and artwork into the session directory.
/// The caller swaps the live database connection to the session file after
/// this succeeds. Wrong passphrase and tampered data both fail here, before
/// any data is exposed.
pub fn unlock_vault_files(
    app_dir: &Path,
    session: &mut VaultSession,
    passphrase: &str,
) -> Result<PathBuf, String> {
    let metadata = read_metadata(app_dir)?;
    let key = key_from_passphrase(&metadata, passphrase)?;
    let session_path = session_db_path(app_dir);
    fs::create_dir_all(session_dir(app_dir))
        .map_err(|e| format!("Could not create vault session directory: {e}"))?;
    if sealed_db_path(app_dir).is_file() {
        let sealed = fs::read(sealed_db_path(app_dir))
            .map_err(|e| format!("Could not read sealed database: {e}"))?;
        let plaintext = open_bytes(&key, &sealed, DB_AAD)?;
        fs::write(&session_path, plaintext)
            .map_err(|e| format!("Could not write session database: {e}"))?;
    } else {
        // Fresh vault: no sealed database yet. The caller creates it.
        let _ = fs::remove_file(&session_path);
    }
    decrypt_artwork(app_dir, &key)?;
    session.key = Some(Zeroizing::new(key));
    session.unlocked = true;
    Ok(session_path)
}

fn artwork_files(app_dir: &Path) -> Vec<PathBuf> {
    let dir = app_dir.join(ARTWORK_DIR);
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

fn decrypt_artwork(app_dir: &Path, key: &[u8; 32]) -> Result<(), String> {
    // Decrypt to .tmp files first, then rename, so a failure does not leave
    // a half-decrypted artwork directory behind.
    let mut staged: Vec<(PathBuf, PathBuf, PathBuf)> = Vec::new();
    for sealed_path in artwork_files(app_dir) {
        let name = sealed_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if !name.ends_with(SEALED_SUFFIX) {
            continue;
        }
        let plain_name = name.trim_end_matches(SEALED_SUFFIX);
        let plain_path = sealed_path.with_file_name(plain_name);
        let tmp_path = sealed_path.with_extension("tmp-vault");
        let sealed = fs::read(&sealed_path).map_err(|e| {
            format!(
                "Could not read sealed artwork {}: {e}",
                sealed_path.display()
            )
        })?;
        let plaintext = open_bytes(key, &sealed, FILE_AAD)?;
        fs::write(&tmp_path, plaintext)
            .map_err(|e| format!("Could not stage artwork {}: {e}", tmp_path.display()))?;
        staged.push((sealed_path, plain_path, tmp_path));
    }
    for (sealed_path, plain_path, tmp_path) in staged {
        fs::rename(&tmp_path, &plain_path)
            .map_err(|e| format!("Could not restore artwork {}: {e}", plain_path.display()))?;
        let _ = fs::remove_file(&sealed_path);
    }
    Ok(())
}

fn seal_artwork(app_dir: &Path, key: &[u8; 32]) -> Result<(), String> {
    for plain_path in artwork_files(app_dir) {
        let name = plain_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if name.ends_with(SEALED_SUFFIX) || name.ends_with("tmp-vault") {
            continue;
        }
        let sealed_path = plain_path.with_file_name(format!("{name}{SEALED_SUFFIX}"));
        seal_file(key, &plain_path, &sealed_path, FILE_AAD)?;
        let _ = fs::remove_file(&plain_path);
    }
    Ok(())
}

/// Lock the vault: checkpoint + seal the session database, seal artwork,
/// wipe the session directory, and zero the in-memory key. The caller must
/// swap the live database connection to an empty in-memory database first
/// (or after, before any further library access), because the session file
/// no longer exists after this returns.
pub fn lock_vault_files(app_dir: &Path, session: &mut VaultSession) -> Result<(), String> {
    let metadata = read_metadata(app_dir)?;
    let key = match session.key.take() {
        Some(key) => key,
        None => {
            // Already locked: still wipe any stray session copy.
            let _ = fs::remove_dir_all(session_dir(app_dir));
            session.unlocked = false;
            return Ok(());
        }
    };
    let session_path = session_db_path(app_dir);
    if session_path.is_file() {
        seal_file(&key, &session_path, &sealed_db_path(app_dir), DB_AAD)?;
        wipe_file(&session_path)?;
        // Remove -wal / -shm companions if SQLite left any behind.
        for suffix in ["-wal", "-shm"] {
            let companion = PathBuf::from(format!("{}{suffix}", session_path.display()));
            let _ = fs::remove_file(&companion);
        }
    }
    seal_artwork(app_dir, &key)?;
    let _ = fs::remove_dir_all(session_dir(app_dir));
    let mut updated = metadata;
    updated.sealed_db = sealed_db_path(app_dir).is_file();
    write_metadata_atomic(app_dir, &updated)?;
    session.unlocked = false;
    Ok(())
}

/// Change the passphrase. Requires the vault to be unlocked (the current
/// session key is present). The sealed database and metadata are re-sealed
/// under the new passphrase; the working session database and artwork stay
/// plaintext while unlocked, and the next lock seals them with the new key.
pub fn change_passphrase(
    app_dir: &Path,
    session: &mut VaultSession,
    old_passphrase: &str,
    new_passphrase: &str,
    new_confirm: &str,
) -> Result<(), String> {
    if !session.unlocked || session.key.is_none() {
        return Err("Unlock the vault before changing its passphrase".into());
    }
    let metadata = read_metadata(app_dir)?;
    // Verify the old passphrase independently of the session key.
    let _ = key_from_passphrase(&metadata, old_passphrase)?;
    check_passphrase(new_passphrase, Some(new_confirm))?;
    let salt = random_bytes::<32>();
    let new_key = derive_key(new_passphrase, &salt)?;
    let (nonce_b64, ciphertext_b64) = make_verification(&new_key)?;
    let session_path = session_db_path(app_dir);
    if session_path.is_file() {
        seal_file(&new_key, &session_path, &sealed_db_path(app_dir), DB_AAD)?;
    }
    let updated = VaultMetadata {
        version: 1,
        salt_b64: B64.encode(salt),
        verification_nonce_b64: nonce_b64,
        verification_ciphertext_b64: ciphertext_b64,
        kdf_m_cost: KDF_M_COST,
        kdf_t_cost: KDF_T_COST,
        kdf_p_cost: KDF_P_COST,
        sealed_db: sealed_db_path(app_dir).is_file(),
        migrated_from_plaintext: metadata.migrated_from_plaintext,
    };
    write_metadata_atomic(app_dir, &updated)?;
    session.key = Some(Zeroizing::new(new_key));
    session.unlocked = true;
    Ok(())
}

/// Export an optional backup file. The backup contains ONLY sealed material
/// (salt, verification tag, sealed database). It is NOT a key file: it
/// cannot unlock anything on its own, and restoring it still requires the
/// same user-selected passphrase. It does not include artwork (posters are
/// regenerated by re-scanning); it restores the database and vault config.
/// The export path must be outside the app data directory.
pub fn export_backup(
    app_dir: &Path,
    session: &VaultSession,
    passphrase: &str,
    export_path: &Path,
) -> Result<(), String> {
    if export_path.starts_with(app_dir) {
        return Err("Choose a backup location outside the vault directory".into());
    }
    let metadata = read_metadata(app_dir)?;
    let key = key_from_passphrase(&metadata, passphrase)?;
    let _ = &key; // Verification only; the backup never contains the key.
    if !session.unlocked {
        return Err("Unlock the vault before exporting a backup".into());
    }
    let session_path = session_db_path(app_dir);
    let sealed_db = if sealed_db_path(app_dir).is_file() {
        fs::read(sealed_db_path(app_dir))
            .map_err(|e| format!("Could not read sealed database: {e}"))?
    } else if session_path.is_file() {
        seal_bytes(
            &key,
            &fs::read(&session_path)
                .map_err(|e| format!("Could not read session database: {e}"))?,
            DB_AAD,
        )?
    } else {
        return Err("No vault database to back up yet".into());
    };
    if sealed_db.len() < 28 {
        return Err("Sealed database is corrupt".into());
    }
    let backup = BackupFile {
        version: 1,
        salt_b64: metadata.salt_b64.clone(),
        verification_nonce_b64: metadata.verification_nonce_b64.clone(),
        verification_ciphertext_b64: metadata.verification_ciphertext_b64.clone(),
        kdf_m_cost: metadata.kdf_m_cost,
        kdf_t_cost: metadata.kdf_t_cost,
        kdf_p_cost: metadata.kdf_p_cost,
        sealed_db_nonce_b64: B64.encode(&sealed_db[4..28]),
        sealed_db_ciphertext_b64: B64.encode(&sealed_db[28..]),
    };
    let raw = serde_json::to_string_pretty(&backup)
        .map_err(|e| format!("Could not serialize backup: {e}"))?;
    write_atomic(export_path, raw.as_bytes())
}

/// Restore a backup file. Requires the same passphrase that created it, and
/// refuses to overwrite an existing vault unless `overwrite` is true.
/// After import the vault is still locked; the user must unlock it.
pub fn import_backup(
    app_dir: &Path,
    passphrase: &str,
    backup_path: &Path,
    overwrite: bool,
) -> Result<(), String> {
    if is_configured(app_dir) && !overwrite {
        return Err(
            "A vault already exists. Confirm overwrite to replace it with this backup".into(),
        );
    }
    let raw =
        fs::read_to_string(backup_path).map_err(|e| format!("Backup file is unreadable: {e}"))?;
    let backup: BackupFile =
        serde_json::from_str(&raw).map_err(|e| format!("Backup file is corrupt: {e}"))?;
    if backup.version != 1 {
        return Err("Unsupported backup version".into());
    }
    let metadata = VaultMetadata {
        version: 1,
        salt_b64: backup.salt_b64.clone(),
        verification_nonce_b64: backup.verification_nonce_b64.clone(),
        verification_ciphertext_b64: backup.verification_ciphertext_b64.clone(),
        kdf_m_cost: backup.kdf_m_cost,
        kdf_t_cost: backup.kdf_t_cost,
        kdf_p_cost: backup.kdf_p_cost,
        sealed_db: true,
        migrated_from_plaintext: false,
    };
    // The passphrase must verify against the backup before anything is written.
    let _ = key_from_passphrase(&metadata, passphrase)?;
    let nonce = B64
        .decode(&backup.sealed_db_nonce_b64)
        .map_err(|_| "Backup database data is corrupt".to_string())?;
    let ciphertext = B64
        .decode(&backup.sealed_db_ciphertext_b64)
        .map_err(|_| "Backup database data is corrupt".to_string())?;
    let mut sealed = Vec::with_capacity(4 + nonce.len() + ciphertext.len());
    sealed.extend_from_slice(MAGIC);
    sealed.extend_from_slice(&nonce);
    sealed.extend_from_slice(&ciphertext);
    write_atomic(&sealed_db_path(app_dir), &sealed)?;
    write_metadata_atomic(app_dir, &metadata)?;
    Ok(())
}

/// Explicit, user-confirmed migration of an existing plaintext library.
/// `confirm` must be exactly "MIGRATE". Steps, in order: (1) set up the
/// vault with the new passphrase, (2) seal the plaintext database and
/// write a sealed backup outside the vault directory, (3) only then delete
/// the plaintext database and seal artwork, (4) unlock into a session
/// database. If any step before (3) fails, the plaintext library is left
/// untouched. After migration completes, the only way back is the sealed
/// backup plus the passphrase — the plaintext copy is gone, by design.
pub fn migrate_vault(
    app_dir: &Path,
    session: &mut VaultSession,
    passphrase: &str,
    confirm: &str,
    backup_path: &Path,
) -> Result<PathBuf, String> {
    if confirm != MIGRATE_CONFIRM {
        return Err("Migration requires explicit confirmation".into());
    }
    if is_configured(app_dir) {
        return Err("A vault is already set up; nothing to migrate".into());
    }
    let plaintext = plaintext_db_path(app_dir);
    if !plaintext.is_file() {
        return Err("No plaintext library to migrate".into());
    }
    if backup_path.starts_with(app_dir) {
        return Err("Choose a backup location outside the vault directory".into());
    }
    if backup_path.is_file() {
        return Err("A backup file already exists at that location; choose a new location".into());
    }
    check_passphrase(passphrase, None)?;
    let salt = random_bytes::<32>();
    let key = derive_key(passphrase, &salt)?;
    let (nonce_b64, ciphertext_b64) = make_verification(&key)?;
    let metadata = VaultMetadata {
        version: 1,
        salt_b64: B64.encode(salt),
        verification_nonce_b64: nonce_b64,
        verification_ciphertext_b64: ciphertext_b64,
        kdf_m_cost: KDF_M_COST,
        kdf_t_cost: KDF_T_COST,
        kdf_p_cost: KDF_P_COST,
        sealed_db: true,
        migrated_from_plaintext: true,
    };
    // Seal first, to a temporary file, before touching the plaintext copy.
    let sealed = seal_bytes(
        &key,
        &fs::read(&plaintext).map_err(|e| format!("Could not read plaintext library: {e}"))?,
        DB_AAD,
    )?;
    if sealed.len() < 28 {
        return Err("Sealed database is corrupt".into());
    }
    let backup = BackupFile {
        version: 1,
        salt_b64: metadata.salt_b64.clone(),
        verification_nonce_b64: metadata.verification_nonce_b64.clone(),
        verification_ciphertext_b64: metadata.verification_ciphertext_b64.clone(),
        kdf_m_cost: metadata.kdf_m_cost,
        kdf_t_cost: metadata.kdf_t_cost,
        kdf_p_cost: metadata.kdf_p_cost,
        sealed_db_nonce_b64: B64.encode(&sealed[4..28]),
        sealed_db_ciphertext_b64: B64.encode(&sealed[28..]),
    };
    let backup_raw = serde_json::to_string_pretty(&backup)
        .map_err(|e| format!("Could not serialize backup: {e}"))?;
    write_atomic(backup_path, backup_raw.as_bytes())?;
    write_atomic(&sealed_db_path(app_dir), &sealed)?;
    write_metadata_atomic(app_dir, &metadata)?;
    // Only now is the plaintext copy removed and artwork sealed.
    let _ = fs::remove_file(&plaintext);
    for suffix in ["-wal", "-shm"] {
        let companion = PathBuf::from(format!("{}{suffix}", plaintext.display()));
        let _ = fs::remove_file(&companion);
    }
    seal_artwork(app_dir, &key)?;
    session.key = Some(Zeroizing::new(key));
    session.unlocked = false;
    Ok(session_db_path(app_dir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cinavault-vault-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn seal_roundtrip_and_wrong_key_and_tamper_fail() {
        let key = derive_key(
            "correct horse battery staple",
            b"test-salt-1234567890123456",
        )
        .unwrap();
        let wrong = derive_key("test-passphrase-wrong", b"test-salt-1234567890123456").unwrap();
        let sealed = seal_bytes(&key, b"hello vault", DB_AAD).unwrap();
        assert_eq!(open_bytes(&key, &sealed, DB_AAD).unwrap(), b"hello vault");
        assert!(open_bytes(&wrong, &sealed, DB_AAD).is_err());
        let mut tampered = sealed.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0xff;
        assert!(open_bytes(&key, &tampered, DB_AAD).is_err());
        // Wrong AAD also fails: authentication covers what is sealed.
        assert!(open_bytes(&key, &sealed, FILE_AAD).is_err());
    }

    #[test]
    fn verification_accepts_correct_and_rejects_wrong_and_lost_key() {
        let dir = temp_dir("verify");
        let mut session = VaultSession::default();
        setup_vault(
            &dir,
            &mut session,
            "test-passphrase-alpha",
            "test-passphrase-alpha",
        )
        .unwrap();
        let metadata = read_metadata(&dir).unwrap();
        let good = key_from_passphrase(&metadata, "test-passphrase-alpha").unwrap();
        assert!(verify_key(&metadata, &good).is_ok());
        // A different passphrase, and a key derived under a different salt,
        // both fail: a lost passphrase cannot be recovered from stored data.
        assert!(key_from_passphrase(&metadata, "test-passphrase-beta").is_err());
        let lost = derive_key("test-passphrase-alpha", b"different-salt-00000000000000").unwrap();
        assert!(verify_key(&metadata, &lost).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn setup_refuses_plaintext_library_and_short_passphrase() {
        let dir = temp_dir("refuse");
        fs::write(dir.join(PLAINTEXT_DB), b"fake sqlite bytes").unwrap();
        let mut session = VaultSession::default();
        assert!(setup_vault(
            &dir,
            &mut session,
            "test-passphrase-valid",
            "test-passphrase-valid"
        )
        .is_err());
        let dir2 = temp_dir("short");
        let mut session2 = VaultSession::default();
        assert!(setup_vault(&dir2, &mut session2, "short", "short").is_err());
        assert!(setup_vault(&dir2, &mut session2, "test-passphrase-valid", "different").is_err());
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&dir2);
    }

    #[test]
    fn unlock_lock_cycle_and_lock_clears_key() {
        let dir = temp_dir("cycle");
        let mut session = VaultSession::default();
        setup_vault(
            &dir,
            &mut session,
            "test-passphrase-cycle",
            "test-passphrase-cycle",
        )
        .unwrap();
        // Simulate a sealed database created at first lock.
        let session_path = session_db_path(&dir);
        fs::create_dir_all(session_dir(&dir)).unwrap();
        fs::write(&session_path, b"library bytes here").unwrap();
        session.key = Some(Zeroizing::new(
            key_from_passphrase(&read_metadata(&dir).unwrap(), "test-passphrase-cycle").unwrap(),
        ));
        session.unlocked = true;
        lock_vault_files(&dir, &mut session).unwrap();
        assert!(!session.unlocked);
        assert!(session.key.is_none());
        assert!(sealed_db_path(&dir).is_file());
        assert!(!session_path.is_file());
        // Wrong passphrase cannot unlock the sealed copy.
        assert!(unlock_vault_files(&dir, &mut session, "test-passphrase-wrong").is_err());
        assert!(!session.unlocked);
        // Correct passphrase unlocks and restores the bytes.
        let restored = unlock_vault_files(&dir, &mut session, "test-passphrase-cycle").unwrap();
        assert!(session.unlocked);
        assert_eq!(fs::read(&restored).unwrap(), b"library bytes here");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn artwork_seal_and_unlock_roundtrip() {
        let dir = temp_dir("artwork");
        let mut session = VaultSession::default();
        setup_vault(
            &dir,
            &mut session,
            "test-passphrase-artwork",
            "test-passphrase-artwork",
        )
        .unwrap();
        let art = dir.join(ARTWORK_DIR);
        fs::create_dir_all(&art).unwrap();
        fs::write(art.join("poster.jpg"), b"fake poster").unwrap();
        let key =
            key_from_passphrase(&read_metadata(&dir).unwrap(), "test-passphrase-artwork").unwrap();
        seal_artwork(&dir, &key).unwrap();
        assert!(!art.join("poster.jpg").is_file());
        assert!(art.join("poster.jpg.vault").is_file());
        decrypt_artwork(&dir, &key).unwrap();
        assert_eq!(fs::read(art.join("poster.jpg")).unwrap(), b"fake poster");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn backup_requires_passphrase_and_restores() {
        let dir = temp_dir("backup-src");
        let mut session = VaultSession::default();
        setup_vault(
            &dir,
            &mut session,
            "test-passphrase-backup",
            "test-passphrase-backup",
        )
        .unwrap();
        let session_path = session_db_path(&dir);
        fs::create_dir_all(session_dir(&dir)).unwrap();
        fs::write(&session_path, b"database to back up").unwrap();
        session.key = Some(Zeroizing::new(
            key_from_passphrase(&read_metadata(&dir).unwrap(), "test-passphrase-backup").unwrap(),
        ));
        session.unlocked = true;
        let export_path = std::env::temp_dir().join("cinavault-vault-test-backup.json");
        let _ = fs::remove_file(&export_path);
        export_backup(&dir, &session, "test-passphrase-backup", &export_path).unwrap();
        let raw = fs::read_to_string(&export_path).unwrap();
        // The backup contains no passphrase and no plaintext database.
        assert!(!raw.contains("test-passphrase-backup"));
        assert!(!raw.contains("database to back up"));
        // Restoring without the passphrase fails; with it, it succeeds.
        let dir2 = temp_dir("backup-dst");
        assert!(import_backup(&dir2, "test-passphrase-wrong", &export_path, false).is_err());
        assert!(!is_configured(&dir2));
        import_backup(&dir2, "test-passphrase-backup", &export_path, false).unwrap();
        assert!(is_configured(&dir2));
        let mut session2 = VaultSession::default();
        let restored = unlock_vault_files(&dir2, &mut session2, "test-passphrase-backup").unwrap();
        assert_eq!(fs::read(&restored).unwrap(), b"database to back up");
        let _ = fs::remove_file(&export_path);
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&dir2);
    }

    #[test]
    fn change_passphrase_old_fails_new_works() {
        let dir = temp_dir("change");
        let mut session = VaultSession::default();
        setup_vault(
            &dir,
            &mut session,
            "test-passphrase-old",
            "test-passphrase-old",
        )
        .unwrap();
        session.key = Some(Zeroizing::new(
            key_from_passphrase(&read_metadata(&dir).unwrap(), "test-passphrase-old").unwrap(),
        ));
        session.unlocked = true;
        change_passphrase(
            &dir,
            &mut session,
            "test-passphrase-old",
            "test-passphrase-new",
            "test-passphrase-new",
        )
        .unwrap();
        let metadata = read_metadata(&dir).unwrap();
        assert!(key_from_passphrase(&metadata, "test-passphrase-old").is_err());
        assert!(key_from_passphrase(&metadata, "test-passphrase-new").is_ok());
        let _ = fs::remove_dir_all(&dir);
    }
}
