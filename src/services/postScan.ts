// Follows the background work a scan schedules (metadata, subtitles, chapter
// thumbnails, poster sync, collections; see src-tauri/src/post_scan.rs) and
// refreshes the library once it is done.
import { invoke } from "@tauri-apps/api/core";

import type { PostScanRun } from "../features/library/libraryAutomation.ts";

export interface PostScanStatus {
  running: boolean;
  queued: number;
  lastRun: PostScanRun | null;
}

let following: Promise<PostScanStatus | null> | null = null;

/** Resolves with the finished run (or null after `timeoutMs`); one poller at a time. */
export function followPostScan(intervalMs = 3000, timeoutMs = 60 * 60 * 1000): Promise<PostScanStatus | null> {
  if (following) return following;
  const started = Date.now();
  following = new Promise<PostScanStatus | null>((resolve) => {
    const tick = async () => {
      try {
        const status = await invoke<PostScanStatus>("post_scan_status");
        if (!status.running && status.queued === 0) return resolve(status);
      } catch {
        return resolve(null);
      }
      if (Date.now() - started > timeoutMs) return resolve(null);
      window.setTimeout(() => void tick(), intervalMs);
    };
    // The worker starts right after the scan returns; give it a moment.
    window.setTimeout(() => void tick(), intervalMs);
  }).finally(() => {
    following = null;
  });
  return following;
}
