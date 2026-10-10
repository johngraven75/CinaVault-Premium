# Unified library and first-run setup

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
- To run the wizard again, open Settings and choose Run setup wizard.
