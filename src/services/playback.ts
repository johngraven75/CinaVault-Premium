// Single entry point for starting playback anywhere in the app. The built-in
// player registers itself here; until it has (or if it declines a title),
// playback goes to the external player through play_media.
import { invoke } from "@tauri-apps/api/core";

import type { MediaItem } from "../store/appStore";

export interface PlayOptions {
  /** Where the request came from, for the activity log ("library", "genre_radio", ...). */
  source?: string;
  /** Shuffle the queue before playing (genre radio). */
  shuffle?: boolean;
  /** Start position in seconds; overrides Auto Resume. */
  startAt?: number;
}

export interface PlayRequest extends PlayOptions {
  queue: MediaItem[];
  index: number;
}

/** Returns true when the built-in player took the request. */
export type PlayerHandler = (request: PlayRequest) => boolean | Promise<boolean>;

let handler: PlayerHandler | null = null;

/** Registers the built-in player; returns an unregister function. */
export function registerPlayer(next: PlayerHandler): () => void {
  handler = next;
  return () => {
    if (handler === next) handler = null;
  };
}

/** Plays `queue[index]`, with the rest of the queue available to Next Up and gapless playback. */
export async function playMedia(queue: MediaItem[], index = 0, options: PlayOptions = {}): Promise<void> {
  const item = queue[index];
  if (!item) throw new Error("Nothing to play");
  if (handler && (await handler({ ...options, queue, index }))) return;
  await invoke("play_media", { filePath: item.file_path });
}

// ------------------------------------------------------------- controls ----
// Transport controls for the already-open built-in player, used by Siri /
// App Intents (siriBridge.ts) and any other out-of-UI caller. The player
// registers a controller while it is mounted; callers get back whether it
// was handled, so a control with no player open is a no-op rather than a throw.

export type PlaybackCommand = "pause" | "resume" | "skip";

/** Returns true when the built-in player acted on the command. */
export type PlayerController = (command: PlaybackCommand) => boolean | Promise<boolean>;

let controller: PlayerController | null = null;

/** Registers the built-in player's transport controls; returns an unregister function. */
export function registerPlayerController(next: PlayerController): () => void {
  controller = next;
  return () => {
    if (controller === next) controller = null;
  };
}

/** Sends a transport command to the open player. Returns true if a player handled it. */
export async function controlPlayback(command: PlaybackCommand): Promise<boolean> {
  if (!controller) return false;
  return (await controller(command)) === true;
}
