// IPC contract for the holo agent's Claude brain. Mirrors the commands and
// payloads in src-tauri/src/ai_agent.rs. The Anthropic API key is write-only
// from here: it goes into agent_set_api_key and never comes back to the WebView.
import { Channel, invoke } from "@tauri-apps/api/core";
import { base64FromDataUrl, imageAttachmentError } from "./holoAgent";
import type { AgentActivityPhase } from "./holoAgent";

export interface AgentStatus {
  configured: boolean;
  keySource: "env" | "keychain" | null;
  model: string;
  models: string[];
}

export type AgentActionKind =
  | { type: "play" }
  | { type: "refreshMetadata" }
  | { type: "rename"; title: string }
  | { type: "setWatched"; watched: boolean }
  | { type: "discoverFolders" }
  | { type: "organizeLibrary"; tasks: string[] };

export interface AgentAction {
  id: string;
  kind: AgentActionKind;
  /** Library item the action targets; null for library-wide actions. */
  mediaId: number | null;
  label: string;
}

export interface AgentReply {
  text: string;
  actions: AgentAction[];
  toolsUsed: string[];
  stopReason: string;
}

export interface AgentImageInput {
  mediaType: string;
  /** Base64 without a data-URL prefix. */
  data: string;
}

export const MODEL_LABELS: Record<string, string> = {
  "claude-opus-5-5": "Claude Opus 5.5 (most capable)",
  "claude-sonnet-5-5": "Claude Sonnet 5.5 (balanced)",
  "claude-haiku-5-5": "Claude Haiku 5.5 (fastest)",
};

export const ANTHROPIC_KEYS_URL = "https://console.anthropic.com/settings/keys";

export const getAgentStatus = () => invoke<AgentStatus>("agent_status");
export const setAgentApiKey = (key: string) => invoke<AgentStatus>("agent_set_api_key", { key });
export const clearAgentApiKey = () => invoke<AgentStatus>("agent_clear_api_key");
export const setAgentModel = (model: string) => invoke<AgentStatus>("agent_set_model", { model });
export const resetAgent = () => invoke<void>("agent_reset");
export const runAgentAction = (actionId: string) => invoke<string>("agent_run_action", { actionId });

/**
 * Sends one message. The back end streams its progress (thinking, searching,
 * acting) over a per-request channel, which drives the head and status line.
 */
export function sendAgentMessage(
  message: string,
  image: AgentImageInput | null,
  onActivity: (phase: AgentActivityPhase, tool: string | null) => void,
): Promise<AgentReply> {
  const channel = new Channel<{ phase: AgentActivityPhase; tool: string | null }>();
  channel.onmessage = (event) => onActivity(event.phase, event.tool);
  return invoke<AgentReply>("agent_chat", { message, image, onActivity: channel });
}

/** Reads a picked or pasted image into the shape agent_chat expects. */
export function readAgentImage(file: File): Promise<AgentImageInput> {
  const problem = imageAttachmentError(file.type, file.size);
  if (problem) return Promise.reject(new Error(problem));
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve({ mediaType: file.type, data: base64FromDataUrl(String(reader.result)) });
    reader.onerror = () => reject(new Error("That image could not be read."));
    reader.readAsDataURL(file);
  });
}

export function errorText(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "Something went wrong";
}
