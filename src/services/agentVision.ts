// Lets the holo agent see images on the free path (no Anthropic key): local
// CLIP describes the picture offline, and with a Hugging Face token Qwen3-VL
// answers the user's question about it through the Hugging Face router.

import { invoke } from "@tauri-apps/api/core";
import {
  POSTER_READ_MODEL,
  analyzePoster,
  describePoster,
  hasHuggingFaceToken,
  readPosterText,
  type InferenceCall,
  type InferenceResult,
  type PosterInsights,
  type PosterRead,
} from "./posterVision.ts";

export interface AgentVisionDeps {
  analyze: (src: string) => Promise<PosterInsights>;
  hasToken: () => Promise<boolean>;
  infer: InferenceCall;
  read: (src: string) => Promise<PosterRead | null>;
}

const defaultDeps: AgentVisionDeps = {
  analyze: (src) => analyzePoster(src),
  hasToken: hasHuggingFaceToken,
  infer: (args) => invoke<InferenceResult>("ai_inference", args),
  read: (src) => readPosterText(src),
};

function messageText(message: unknown): string | null {
  if (typeof message === "string") return message.trim() || null;
  if (Array.isArray(message)) {
    const text = message
      .map((part) => (part && typeof part === "object" && "text" in part ? String((part as { text: unknown }).text) : ""))
      .join("")
      .trim();
    return text || null;
  }
  return null;
}

/**
 * Answers about an attached image. With a question and a token the cloud model
 * answers it; without a question it reads the poster. Local vision always
 * adds its own take, so the agent still answers offline.
 */
export async function seeImage(dataUrl: string, question: string, deps: AgentVisionDeps = defaultDeps): Promise<string> {
  const [insights, token] = await Promise.all([
    deps.analyze(dataUrl).catch(() => null),
    deps.hasToken().catch(() => false),
  ]);
  const ask = question.trim();
  let cloudAnswer: string | null = null;
  let read: PosterRead | null = null;
  if (token) {
    if (ask) {
      const result = await deps.infer({ input: ask, model: POSTER_READ_MODEL, imageUrl: dataUrl }).catch(() => null);
      cloudAnswer = result?.status === "success" ? messageText(result.message) : null;
    } else {
      read = await deps.read(dataUrl).catch(() => null);
    }
  }
  const local = insights ? describePoster(insights, read) : null;
  if (cloudAnswer) return local ? `${cloudAnswer}\n\n${local}` : cloudAnswer;
  if (local) {
    return token || !ask
      ? local
      : `${local} Add a free Hugging Face token in AI Configure and I can answer questions about what's written on it.`;
  }
  return "I couldn't look at that image. The local vision model may still be loading; try again in a moment.";
}
