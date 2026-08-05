# External-drive root scan hotfix

## Reproduction

1. Add `D:\` as an enabled source with source type `drive`.
2. Start a single-source scan.
3. The UI appears not to scan because the backend performs a complete recursive `WalkDir` collection before `SCAN_TOTAL` and `SCAN_CURRENT` begin advancing.

## Root cause

`collect_media_files` walks and stores the entire drive before `scan_directory` updates progress or writes any records. On a large external drive, inaccessible/system directories and a very large directory tree can make the scan look frozen for a long time.

## Required fix

- Stream entries from `WalkDir` directly into indexing instead of collecting the whole drive first.
- Update progress while traversing, not only after collection.
- Skip common Windows/system directories and recycle-bin/system-volume paths.
- Continue past permission and transient I/O errors while reporting them as warnings.
- Add regression coverage for a Windows drive-root source such as `D:\`.

## Immediate workaround

Add the actual media folders as separate sources, for example `D:\Movies`, `D:\TV`, and `D:\Music`, rather than scanning the entire `D:\` root.
