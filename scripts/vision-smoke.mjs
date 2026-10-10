#!/usr/bin/env node
// End-to-end check of the bundled offline vision model: loads CLIP from
// public/models/ exactly as the app does (same id, revision and 8-bit dtype,
// no network), embeds two synthetic images and two captions, and fails unless
// each image is closest to its own caption. Run after `npm run fetch:ai-models`.
//
// Usage: node scripts/vision-smoke.mjs

import { existsSync } from "node:fs";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import {
  AutoProcessor,
  AutoTokenizer,
  CLIPTextModelWithProjection,
  CLIPVisionModelWithProjection,
  RawImage,
  env,
} from "@huggingface/transformers";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const MODEL_ID = "Xenova/clip-vit-base-patch32";
const REVISION = "d15189d7028b43f1d3e65039190477f6af591c2a";
const LOCAL = join(ROOT, "public", "models");

if (!existsSync(join(LOCAL, ...MODEL_ID.split("/"), "onnx", "vision_model_quantized.onnx"))) {
  console.error("Vision model is not bundled. Run `npm run fetch:ai-models` first.");
  process.exit(1);
}

env.localModelPath = `${LOCAL}/`;
env.allowLocalModels = true;
env.allowRemoteModels = false;

function solid([r, g, b], size = 224) {
  const data = new Uint8ClampedArray(size * size * 3);
  for (let i = 0; i < data.length; i += 3) data.set([r, g, b], i);
  return new RawImage(data, size, size, 3);
}

function normalize(vector) {
  const norm = Math.hypot(...vector) || 1;
  return vector.map((v) => v / norm);
}

const dot = (a, b) => a.reduce((sum, v, i) => sum + v * b[i], 0);

const started = Date.now();
const options = { dtype: "q8", revision: REVISION };
const [tokenizer, processor, textModel, visionModel] = await Promise.all([
  AutoTokenizer.from_pretrained(MODEL_ID, { revision: REVISION }),
  AutoProcessor.from_pretrained(MODEL_ID, { revision: REVISION }),
  CLIPTextModelWithProjection.from_pretrained(MODEL_ID, options),
  CLIPVisionModelWithProjection.from_pretrained(MODEL_ID, options),
]);

const captions = ["a plain bright red square", "a plain bright blue square"];
const texts = tokenizer(captions, { padding: true, truncation: true });
const { text_embeds } = await textModel(texts);
const [dims] = text_embeds.dims.slice(-1);
const textVectors = captions.map((_, i) => normalize(Array.from(text_embeds.data.slice(i * dims, (i + 1) * dims))));

const images = [solid([230, 20, 20]), solid([20, 40, 230])];
let failures = 0;
for (const [index, image] of images.entries()) {
  const inputs = await processor(image);
  const { image_embeds } = await visionModel(inputs);
  const vector = normalize(Array.from(image_embeds.data));
  const scores = textVectors.map((text) => dot(vector, text));
  const best = scores.indexOf(Math.max(...scores));
  const ok = best === index && vector.length === 512;
  if (!ok) failures += 1;
  console.log(`${ok ? "ok" : "FAIL"} image ${index}: ${captions.map((c, i) => `${c}=${scores[i].toFixed(3)}`).join(", ")}`);
}

console.log(`Vision model end-to-end ${failures ? "FAILED" : "passed"} in ${Date.now() - started} ms.`);
process.exit(failures ? 1 : 0);
