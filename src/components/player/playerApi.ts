// Back-end calls the built-in player makes (player_stream.rs, user_data.rs, player.rs).
import { invoke } from "@tauri-apps/api/core";

import type { ProbeInfo, SavedProgress } from "../../services/playerLogic";

/** player_stream.rs PlayerSource. */
export interface PlayerSource {
  mediaId: number;
  filePath: string;
  probe: ProbeInfo | null;
  probeError: string | null;
  /** Loopback transcode URL; null when ffmpeg is missing or Force Direct Play is on. */
  transcodeUrl: string | null;
  forceDirectPlay: boolean;
  ffmpegAvailable: boolean;
}

export interface TranscodeStatus {
  ffmpegAvailable: boolean;
  hardwareEnabled: boolean;
  usableHardware: string[];
  encoder: string;
}

export const preparePlayback = (mediaId: number | undefined, filePath: string) =>
  invoke<PlayerSource>("player_prepare", { mediaId: mediaId ?? null, filePath });

export const transcodeStatus = () => invoke<TranscodeStatus>("player_transcode_status");

export const loadProgress = (mediaId: number) =>
  invoke<SavedProgress | null>("playback_progress_get", { mediaId }).catch(() => null);

export const saveProgress = (mediaId: number, position: number, duration: number) =>
  invoke("playback_progress_save", { mediaId, position, duration }).catch((error) =>
    console.warn("Playback progress not saved:", error),
  );

export const recordActivity = (kind: string, title: string, detail: Record<string, unknown>) =>
  invoke("activity_record", { kind, title, detail }).catch((error) =>
    console.warn("Activity not recorded:", error),
  );

/** Hands a title to the external player (Settings > Default Player). */
export const openExternally = (filePath: string) => invoke("play_media", { filePath });
