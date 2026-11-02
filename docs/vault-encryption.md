# CinaVault Premium — App Vault encryption

Scope: **app vault only**. This encrypts CinaVault's own library database
and generated artwork under the app data directory. It is not whole-drive or
whole-filesystem encryption, and it does not encrypt arbitrary user files.

## The one rule, plainly

**Your passphrase is the required unlock credential.** At first-run setup
you choose your own passphrase, and that same passphrase is required on
every launch to open the vault. A system-generated key file is never the
required or sole method, and no key file can replace or bypass the
passphrase.

> If you lose your passphrase, your vault cannot be recovered. There is no
> backdoor, no recovery key held by CinaVault, and no plaintext fallback.
> An optional backup file you export yourself still requires your
> passphrase to restore — it is not a key that unlocks the vault on its own.

## Design

- **Cipher:** XChaCha20-Poly1305 AEAD (`chacha20poly1305` crate).
  Every sealed object uses a fresh random 24-byte nonce and an
  authenticated header (AAD) that binds the ciphertext to its purpose:
  `cinavault-vault-verification-v1`, `cinavault-vault-db-v1`, or
  `cinavault-vault-file-v1`. Tampering or a wrong passphrase fails
  authentication; no partial plaintext is ever produced.
- **Key derivation:** Argon2id, m=19456 KiB, t=2, p=1, 32-byte output.
  A fresh random salt is generated per vault and stored in `vault.json`
  alongside a verification tag (an AEAD seal of a fixed check string).
  Only the salt and the verification tag are stored — never the
  passphrase, never the derived key, never in a log or test fixture.
- **At-rest layout** (under the app data directory):
  - `vault.json` — version, KDF parameters, salt, verification tag.
    Not secret on its own; useless without the passphrase.
  - `cinavault.db.vault` — the sealed database while locked.
  - `artwork/**/*.vault` — sealed artwork while locked; plaintext
    artwork files are removed while locked.
  - `.vault-session/` — the decrypted working copy while unlocked
    (`cinavault.db` plus WAL companions). On lock it is sealed back,
    zero-overwritten, and deleted.
  - `cinavault.db` (plaintext) — only in legacy mode, or as the session
    copy while unlocked.
- **Key lifetime:** the derived key lives only in memory, wrapped in a
  `Zeroizing<[u8; 32]>` that is cleared on lock. The OS keyring
  (`secure_credentials`) is deliberately **not** used to store the vault
  key, so a locked vault cannot be opened from a stored credential.
- **Startup gate:** if a vault is configured, or this is a true first run
  (no vault and no library), the app starts with an empty in-memory
  database and the frontend renders the vault gate instead of any library
  tab. Database-dependent startup initialisation that ran against that
  placeholder is re-run on unlock (tables and the metadata provider
  catalogue). The embedded native server is configured with the session
  database path at startup and must not be started while locked.
- **Wrong key:** unlock fails cleanly. The vault stays locked, the sealed
  files stay sealed, and no library data, media, or partial database is
  exposed.

## First run, legacy libraries, and migration

- **First run:** setup is mandatory before any library or database access.
  Setup refuses to run if a plaintext database already exists — that is
  what migration is for. Passphrases must be at least 8 characters and
  the confirmation must match.
- **Existing plaintext libraries are not silently encrypted.** A legacy
  library (plaintext `cinavault.db`, no vault) keeps opening normally
  until the user explicitly migrates it from the Settings vault panel.
- **Migration** (explicit, user-confirmed):
  1. The user chooses a passphrase and a backup path **outside** the
     vault folder, and types `MIGRATE` as confirmation.
  2. The app seals the plaintext database to that backup path first.
  3. Only then does it write the sealed database and vault metadata,
     delete the plaintext database and its WAL companions, and seal the
     artwork.
  4. If any step before plaintext deletion fails, the plaintext library
     is left untouched.
  5. Migration is reversible **only** via that backup, and only with the
     passphrase: import the backup, then unlock. After the plaintext
     copy is deleted there is no other way back.

## Key management

Commands and UI (Settings → App Vault): `vault_status`, `vault_setup`,
`vault_unlock`, `vault_lock`, `vault_change_passphrase`,
`vault_export_backup`, `vault_import_backup`, `vault_migrate`.

- **Lock** clears the key from memory, checkpoints and seals the session
  database, seals artwork, and wipes the session copy. It is idempotent.
- **Change passphrase** requires the vault unlocked and the old passphrase
  verified independently; it writes a new salt and verification tag and
  re-seals the session database. The next lock seals everything with the
  new passphrase; the old passphrase then fails.
- **Export backup** requires the vault unlocked and the passphrase
  verified. The backup is a JSON file containing the salt, the
  verification tag, and the sealed database only. It contains no key and
  no plaintext, it cannot unlock anything on its own, and it does **not**
  include artwork (artwork is regenerable by re-scanning). Export paths
  inside the vault folder are refused.
- **Import backup** verifies the passphrase against the backup before
  writing anything, refuses to overwrite an existing vault unless
  explicitly told to, and leaves the imported vault locked.

## Limitations (stated, not hidden)

- Artwork in a backup is not included; re-scan after a restore.
- The embedded native server must not be started while the vault is
  locked; after unlock it opens the session database.
- Drive media listing/ingestion is a separate feature and is not changed
  by the vault; a vault user still needs a local folder or a future
  listing command for cloud media.
- This feature has not been tested end-to-end in a Tauri run or on
  Windows; see the pull request body for the verified / unverified list.
