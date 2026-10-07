# Unified library, first-run setup and CinaVault Plus

## Unified library

Every source (local drives, NAS, cloud, network shares) is scanned into one `media_items` table. The `get_unified_library` command (`src-tauri/src/library_unify.rs`) folds every copy of a work into one card.

- **Work identity:**
  - If the title has a TMDb or IMDb id, the key is `tmdb:<id>` or `imdb:<id>`.
  - Otherwise the key is the normalized title, the year and the media group. Normalization strips release noise (1080p, x265, WEB-DL, groups, extensions) and folds accents and punctuation.
  - Episodes include `sXXeYY`, so two different episodes never merge.
  - A movie and an adult scene with the same title stay separate.
- **Best copy:** the card shows the copy with the highest resolution, then the largest file, then the one with a poster. Missing poster, overview, rating and year are filled in from the other copies.
- **Duplicate finder:** `find_duplicates` modes are `name_size`, `size`, `name`, `work` (same work) and `content`. `content` is a fast fingerprint: SHA-256 of the file size plus its first, middle and last 4 MiB.
  - Remove and quarantine work again; they were querying a column that doesn't exist.

## First-run setup

On first launch a wizard asks once for the metadata keys that providers issue per account: TMDb, OMDb and Fanart, plus ThePornDB and StashDB (Premium edition only). Each key can be tested and is stored in the OS keyring.

- Keyless providers (IAFD, PGMA bridge, TVMaze, Cinemeta) and the local AI model need nothing.
- **Skip** is always available.
- To run the wizard again, use Settings or Account & Plan.

## CinaVault Plus ($9.99/month)

This follows the design from John's Manus session. The back end is the enforcement point: locked commands refuse with `PAYWALL:<feature>:<message>`, and the embedded server answers HTTP 402.

| Feature | Free | Plus |
|---|---|---|
| Local drives, playback, unified library, home-network streaming | Yes | Yes |
| NAS, cloud and network-share libraries (`external_libraries`) | No | Yes |
| Downloads (`downloads`) | No | Yes |
| Adult metadata providers (`adult_metadata`) | No | Yes |
| Remote access from another network (`remote_access`) | No | Yes |

- **Trial:** a new install starts on Free. Each install can start a 30-day Plus trial once, from the Upgrade panel or the Account & Plan tab. The length is `TRIAL_DAYS` in `src-tauri/src/entitlements.rs`.
- **Licenses:** a license is an offline Ed25519-signed token, `CVL1.<payload>.<signature>`. A build verifies tokens with the public key baked in at build time. If there is no key, the build accepts no license at all (it fails closed); the trial still works.

### Setting up licensing (one time, owner only)

1. Generate a keypair. Keep the private key outside the repo and never commit it:
   `node scripts/issue-license.mjs --generate-keypair --out ~/cinavault-license-private.pem`
2. In the GitHub repo settings, open **Variables** (not Secrets, because these values are public) and set:
   - `CINAVAULT_LICENSE_PUBLIC_KEY`: the base64 public key printed in step 1.
   - `CINAVAULT_CHECKOUT_URL`: your hosted checkout link, for example a Stripe Payment Link for $9.99/month.
3. The Windows installer workflow bakes both values into the build.
4. To issue a license after a payment:
   `node scripts/issue-license.mjs --email buyer@example.com --months 1 --key ~/cinavault-license-private.pem`
   Send the printed token to the buyer. They paste it into **Account & Plan**.
