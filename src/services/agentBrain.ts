// IPC contract for the holo agent's Hugging Face brain. Mirrors the commands and
// payloads in src-tauri/src/ai_agent.rs. The head runs on a free, open-weight
// multimodal reasoning model served through the Hugging Face router — there is no
// Claude on this path. The Hugging Face token is write-only from here: it goes
// into agent_set_hf_token and never comes back to the WebView.
import { Channel, invoke } from "@tauri-apps/api/core";
import { base64FromDataUrl, imageAttachmentError } from "./holoAgent";
import type { AgentActivityPhase } from "./holoAgent";

export interface AgentStatus {
  configured: boolean;
  /** Where the Hugging Face token came from: "settings", "env", "cache", or null when none. */
  keySource: "settings" | "env" | "cache" | null;
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
  "Qwen/Qwen3-VL-8B-Thinking": "Qwen3-VL 8B Thinking (reasoning + vision)",
  "Qwen/Qwen3-VL-8B-Instruct": "Qwen3-VL 8B Instruct (fast, vision)",
  "Qwen/Qwen2.5-VL-7B-Instruct": "Qwen2.5-VL 7B Instruct (vision)",
  "dphn/dolphin-2.9-llama3-8b": "Dolphin 2.9 8B — Uncensored (text only)",
  "mlabonne/NeuralDaredevil-8B-abliterated": "NeuralDaredevil 8B — Uncensored (text only)",
};

/** Models that answer without content filtering; they are text-only (no image understanding). */
export const UNCENSORED_MODELS: ReadonlySet<string> = new Set([
  "dphn/dolphin-2.9-llama3-8b",
  "mlabonne/NeuralDaredevil-8B-abliterated",
]);

export const HF_TOKENS_URL = "https://huggingface.co/settings/tokens";

export const getAgentStatus = () => invoke<AgentStatus>("agent_status");
export const setAgentHfToken = (token: string) => invoke<AgentStatus>("agent_set_hf_token", { token });
export const clearAgentHfToken = () => invoke<AgentStatus>("agent_clear_hf_token");
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
