export type AiProviderStatus = {
  provider: "local" | "huggingface";
  model: string;
  requiresToken: boolean;
  enabled: boolean;
};

export const DEFAULT_LOCAL_AI_MODEL = "cinavault-local-media-agent";

export function hasValidHuggingFaceToken(token?: string | null): boolean {
  return Boolean(
    token && token.trim().startsWith("hf_") && token.trim().length > 20,
  );
}

export function getSafeAiProvider(token?: string | null): AiProviderStatus {
  if (hasValidHuggingFaceToken(token)) {
    return {
      provider: "huggingface",
      model: "katanemo/Arch-Router-1.5B",
      requiresToken: true,
      enabled: true,
    };
  }

  return {
    provider: "local",
    model: DEFAULT_LOCAL_AI_MODEL,
    requiresToken: false,
    enabled: true,
  };
}

export function shouldSuppressUnauthorizedModelError(
  statusCode: number,
): boolean {
  return statusCode === 401 || statusCode === 403;
}
