// Local, free AI vision for CinaVault.
//
// Runs OpenAI CLIP (ViT-B/32, 8-bit quantized ONNX export by Xenova) inside the
// WebView with Transformers.js. No API key, no account, no paid service.
//
// Weight loading order:
//   1. /models/<MODEL_ID>/...  bundled into the installer by `npm run fetch:ai-models`
//   2. huggingface.co CDN      fallback for dev builds without bundled weights;
//                              cached by the browser Cache API after first use.
// ONNX Runtime: in production builds Vite emits the WASM binary as a hashed
// asset and the JS factory is inlined in the onnxruntime-web bundle, so the
// runtime is fully offline. Dev (`vite`) keeps the Transformers.js CDN default
// because Vite's dep pre-bundling breaks ORT's `new URL(..., import.meta.url)`.
//
// Transformers.js (~1.3 MB JS + ORT) is imported lazily so it never touches the
// app's first paint.

import { cosineSimilarity, normalize, softmax } from "./visionMath.ts";

export const VISION_MODEL_ID = "Xenova/clip-vit-base-patch32";
export const VISION_MODEL_DTYPE = "q8" as const;
/** Hub commit the bundled weights come from; the remote fallback pins it too (scripts/fetch-ai-models.mjs). */
export const VISION_MODEL_REVISION = "d15189d7028b43f1d3e65039190477f6af591c2a";
export const LOCAL_MODEL_PATH = "/models/";
/** CLIP's learned logit scale (exp(4.6052) = 100). */
export const CLIP_LOGIT_SCALE = 100;

export type VisionDevice = "webgpu" | "wasm";
export type VisionState = "idle" | "loading" | "ready" | "error";
export type ImageSource = string | URL | Blob | HTMLCanvasElement | OffscreenCanvas;

export interface VisionStatus {
  state: VisionState;
  device: VisionDevice | null;
  modelSource: "bundled" | "remote" | null;
  error: string | null;
}

export interface LabelScore {
  label: string;
  score: number;
}

type TransformersModule = typeof import("@huggingface/transformers");

interface VisionRuntime {
  lib: TransformersModule;
  tokenizer: Awaited<ReturnType<TransformersModule["AutoTokenizer"]["from_pretrained"]>>;
  processor: Awaited<ReturnType<TransformersModule["AutoProcessor"]["from_pretrained"]>>;
  textModel: Awaited<ReturnType<TransformersModule["CLIPTextModelWithProjection"]["from_pretrained"]>>;
  visionModel: Awaited<ReturnType<TransformersModule["CLIPVisionModelWithProjection"]["from_pretrained"]>>;
  device: VisionDevice;
}

// ── Small LRU ────────────────────────────────────────────────────────────────

export class LruCache<K, V> {
  private readonly map = new Map<K, V>();
  constructor(private readonly capacity: number) {}

  get(key: K): V | undefined {
    const value = this.map.get(key);
    if (value === undefined) return undefined;
    this.map.delete(key);
    this.map.set(key, value);
    return value;
  }

  set(key: K, value: V): void {
    if (this.map.has(key)) this.map.delete(key);
    this.map.set(key, value);
    while (this.map.size > this.capacity) {
      const oldest = this.map.keys().next().value as K;
      this.map.delete(oldest);
    }
  }

  has(key: K): boolean {
    return this.map.has(key);
  }

  delete(key: K): void {
    this.map.delete(key);
  }

  clear(): void {
    this.map.clear();
  }

  get size(): number {
    return this.map.size;
  }
}

// 512 floats * 4 bytes = 2 KiB per entry; 1000 entries ≈ 2 MiB.
const imageEmbeddingCache = new LruCache<string, Float32Array>(1000);
const textEmbeddingCache = new LruCache<string, Float32Array>(2000);
const inflightImages = new Map<string, Promise<Float32Array>>();

// ── Loader (single flight) ───────────────────────────────────────────────────

const status: VisionStatus = { state: "idle", device: null, modelSource: null, error: null };
let runtimePromise: Promise<VisionRuntime> | null = null;
let runtime: VisionRuntime | null = null;
const listeners = new Set<(status: VisionStatus) => void>();

function setStatus(patch: Partial<VisionStatus>) {
  Object.assign(status, patch);
  const snapshot = { ...status };
  for (const listener of listeners) {
    try {
      listener(snapshot);
    } catch {
      // listener errors never break the loader
    }
  }
}

export function getVisionStatus(): VisionStatus {
  return { ...status };
}

export function onVisionStatus(listener: (status: VisionStatus) => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function isVisionReady(): boolean {
  return runtime !== null;
}

/**
 * True when `url` serves a real file. Dev servers and SPA hosts answer missing
 * paths with index.html (200, text/html), so HTML responses count as missing.
 */
async function assetExists(url: string): Promise<boolean> {
  try {
    const response = await fetch(url, { method: "GET", cache: "no-store" });
    if (!response.ok) return false;
    const type = response.headers.get("content-type") ?? "";
    // Drain the small body; the caller only needs a yes/no.
    void response.body?.cancel().catch(() => undefined);
    return !type.includes("text/html");
  } catch {
    return false;
  }
}

function isDevBuild(): boolean {
  // Optional chaining keeps this safe under node:test, where import.meta.env is undefined.
  return import.meta.env?.DEV === true;
}

async function detectDevice(): Promise<VisionDevice> {
  try {
    const gpu = (globalThis.navigator as Navigator & { gpu?: { requestAdapter(): Promise<unknown> } })?.gpu;
    if (!gpu) return "wasm";
    const adapter = await gpu.requestAdapter();
    return adapter ? "webgpu" : "wasm";
  } catch {
    return "wasm";
  }
}

async function configureEnvironment(lib: TransformersModule): Promise<"bundled" | "remote"> {
  const { env } = lib;
  env.localModelPath = LOCAL_MODEL_PATH;
  env.allowRemoteModels = true;
  env.useBrowserCache = typeof caches !== "undefined";

  const bundled = await assetExists(`${LOCAL_MODEL_PATH}${VISION_MODEL_ID}/config.json`);
  // Only look locally when the weights were bundled; otherwise a dev server's
  // index.html fallback would be parsed as JSON and break loading.
  env.allowLocalModels = bundled;

  const wasm = env.backends?.onnx?.wasm as
    | { wasmPaths?: unknown; proxy?: boolean; numThreads?: number }
    | undefined;
  if (wasm) {
    // Transformers.js points wasmPaths at cdn.jsdelivr.net on import. Clearing it
    // makes ORT use the bundled factory + the WASM file Vite emitted, which also
    // avoids blob: script URLs that a strict `script-src 'self'` CSP blocks.
    if (!isDevBuild()) {
      wasm.wasmPaths = undefined;
      env.useWasmCache = false;
    }
    // WebView2/WKWebView are not cross-origin isolated, so threads are unavailable.
    if (typeof crossOriginIsolated === "undefined" || !crossOriginIsolated) wasm.numThreads = 1;
  }
  return bundled ? "bundled" : "remote";
}

async function loadRuntime(device: VisionDevice, lib: TransformersModule): Promise<VisionRuntime> {
  const revision = VISION_MODEL_REVISION;
  const options = { device, dtype: VISION_MODEL_DTYPE, revision };
  const [tokenizer, processor, textModel, visionModel] = await Promise.all([
    lib.AutoTokenizer.from_pretrained(VISION_MODEL_ID, { revision }),
    lib.AutoProcessor.from_pretrained(VISION_MODEL_ID, { revision }),
    lib.CLIPTextModelWithProjection.from_pretrained(VISION_MODEL_ID, options),
    lib.CLIPVisionModelWithProjection.from_pretrained(VISION_MODEL_ID, options),
  ]);
  return { lib, tokenizer, processor, textModel, visionModel, device };
}

async function createRuntime(): Promise<VisionRuntime> {
  setStatus({ state: "loading", error: null });
  const lib = await import("@huggingface/transformers");
  const modelSource = await configureEnvironment(lib);
  const preferred = await detectDevice();
  try {
    return await loadRuntime(preferred, lib);
  } catch (error) {
    if (preferred === "wasm") throw error;
    console.warn("[localVision] WebGPU init failed, falling back to WASM:", error);
    return loadRuntime("wasm", lib);
  } finally {
    setStatus({ modelSource });
  }
}

/** Loads the model once; concurrent callers share the same promise. Rejects on failure. */
export function ensureVision(): Promise<VisionRuntime> {
  if (runtime) return Promise.resolve(runtime);
  if (!runtimePromise) {
    runtimePromise = createRuntime()
      .then((loaded) => {
        runtime = loaded;
        setStatus({ state: "ready", device: loaded.device, error: null });
        return loaded;
      })
      .catch((error: unknown) => {
        runtimePromise = null; // allow a later retry
        setStatus({ state: "error", error: error instanceof Error ? error.message : String(error) });
        throw error;
      });
  }
  return runtimePromise;
}

/**
 * Starts loading the model in the background. Safe to call at app boot:
 * never throws, never blocks, resolves to whether vision became ready.
 */
export function warmUpVision(): Promise<boolean> {
  return ensureVision().then(
    () => true,
    (error) => {
      console.warn("[localVision] warm-up failed; vision features stay disabled until retry:", error);
      return false;
    },
  );
}

// ── Embeddings ───────────────────────────────────────────────────────────────

function cacheKey(src: ImageSource): string | null {
  if (typeof src === "string") return src;
  if (src instanceof URL) return src.href;
  return null; // Blobs and canvases are not cached
}

function firstRow(tensor: { data: ArrayLike<number>; dims: number[] }): Float32Array {
  const width = tensor.dims[tensor.dims.length - 1];
  return normalize(Array.prototype.slice.call(tensor.data, 0, width) as number[]);
}

/** L2-normalized 512-d CLIP image embedding. Cached by src (LRU). */
export async function embedImage(src: ImageSource): Promise<Float32Array> {
  const key = cacheKey(src);
  if (key) {
    const cached = imageEmbeddingCache.get(key);
    if (cached) return cached;
    const pending = inflightImages.get(key);
    if (pending) return pending;
  }
  const task = (async () => {
    const vision = await ensureVision();
    const image = await vision.lib.RawImage.read(src as Parameters<TransformersModule["RawImage"]["read"]>[0]);
    const inputs = await vision.processor(image);
    const { image_embeds } = await vision.visionModel(inputs);
    const embedding = firstRow(image_embeds);
    if (key) imageEmbeddingCache.set(key, embedding);
    return embedding;
  })();
  if (!key) return task;
  inflightImages.set(key, task);
  try {
    return await task;
  } finally {
    inflightImages.delete(key);
  }
}

/** L2-normalized CLIP text embeddings for several strings in one batch. */
export async function embedTexts(texts: readonly string[]): Promise<Float32Array[]> {
  const results: (Float32Array | undefined)[] = texts.map((text) => textEmbeddingCache.get(text));
  const missing = [...new Set(texts.filter((_, index) => !results[index]))];
  if (missing.length > 0) {
    const vision = await ensureVision();
    const inputs = vision.tokenizer(missing, { padding: true, truncation: true });
    const { text_embeds } = await vision.textModel(inputs);
    const width = text_embeds.dims[text_embeds.dims.length - 1];
    const data = text_embeds.data as Float32Array;
    missing.forEach((text, row) => {
      textEmbeddingCache.set(text, normalize(data.subarray(row * width, (row + 1) * width)));
    });
  }
  return texts.map((text, index) => results[index] ?? (textEmbeddingCache.get(text) as Float32Array));
}

/** L2-normalized 512-d CLIP text embedding. */
export async function embedText(text: string): Promise<Float32Array> {
  const [embedding] = await embedTexts([text]);
  return embedding;
}

/** Zero-shot classification: probability per label (sums to 1), best first. */
export async function classifyImage(src: ImageSource, labels: readonly string[]): Promise<LabelScore[]> {
  if (labels.length === 0) return [];
  const [imageEmbedding, labelEmbeddings] = await Promise.all([embedImage(src), embedTexts(labels)]);
  const similarities = labelEmbeddings.map((embedding) => cosineSimilarity(imageEmbedding, embedding));
  const probabilities = softmax(similarities, CLIP_LOGIT_SCALE);
  return labels
    .map((label, index) => ({ label, score: probabilities[index] }))
    .sort((left, right) => right.score - left.score);
}

/** Drops cached embeddings (e.g. after a poster file was replaced on disk). */
export function forgetImageEmbedding(src?: string): void {
  if (src === undefined) imageEmbeddingCache.clear();
  else imageEmbeddingCache.delete(src);
}
