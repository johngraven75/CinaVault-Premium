import { invoke } from "@tauri-apps/api/core";
import {
  arePermanentMediaPluginsReady,
  getStartupMediaPlugins,
} from "../plugins/permanentMediaPlugins";

export function initializePermanentMediaPluginsAtStartup() {
  return {
    ready: arePermanentMediaPluginsReady(),
    startupPlugins: getStartupMediaPlugins(),
    message:
      "Permanent media plugins are installed, enabled, configured, and initialized for startup.",
  };
}

export type MediaToolStartupResult = {
  type: "media_tools_startup";
  status: "ready" | "missing_tools";
  ready: boolean;
  automatic: true;
  authorization_prompt_required: false;
  tools: Array<{ id: string; installed: boolean; version?: string | null }>;
};

export async function ensurePermanentMediaPluginsAtStartup(): Promise<MediaToolStartupResult> {
  return invoke<MediaToolStartupResult>("ensure_media_tools");
}
