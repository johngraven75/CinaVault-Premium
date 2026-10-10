#!/usr/bin/env node
// Downloads the local AI vision model (CLIP ViT-B/32, 8-bit ONNX) into
// public/models/<id>/ so `vite build` copies it into dist/ and the Tauri
// installer, and vision works offline on first launch. No Hugging Face token is
// needed: the model is public. (The ONNX Runtime WASM binary needs no fetch:
// Vite emits it from node_modules/onnxruntime-web as a hashed asset.)
//
// Usage:  node scripts/fetch-ai-models.mjs [--force] [--check]
//   --force  re-download every file
//   --check  verify only; exit 1 if anything is missing (no network)
// Env:    HF_ENDPOINT     alternate Hub host/mirror (default https://huggingface.co)
//
// Supply chain: files come from one pinned Hub commit (REVISION) and every
// file is checked against a digest recorded here before it is accepted, so a
// changed upstream repo or a tampered mirror fails the build instead of
// shipping. LFS weights are checked by SHA-256 (the Hub's LFS oid); small JSON
// files by their git blob SHA-1 at that commit. To move to a new revision,
// update REVISION and MODEL_FILES together from
// /api/models/<id>/tree/<commit>?recursive=1.
//
// Idempotent: files already present with the expected digest are skipped.
// Resumable: downloads stream to <file>.part and resume with an HTTP Range
// request when a partial file exists.

import { createWriteStream } from "node:fs";
import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { mkdir, rename, rm, stat } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";

function trimTrailingSlashes(url) {
  let end = url.length;
  while (end > 0 && url[end - 1] === "/") end -= 1;
  return url.slice(0, end);
}

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const MODEL_ID = "Xenova/clip-vit-base-patch32";
const ENDPOINT = trimTrailingSlashes(process.env.HF_ENDPOINT || "https://huggingface.co");
// Xenova/clip-vit-base-patch32 at the 2025-07-08 commit.
export const REVISION = "d15189d7028b43f1d3e65039190477f6af591c2a";
const MODEL_DIR = join(ROOT, "public", "models", ...MODEL_ID.split("/"));

// size in bytes plus a digest: `sha256` (LFS oid) or `gitSha1` (git blob oid).
export const MODEL_FILES = {
  "config.json": { size: 4524, gitSha1: "e79bad158b04c235740f9b2ec320b17f4030f7a5" },
  "preprocessor_config.json": { size: 520, gitSha1: "e0675735fd0052745f2f7b7291e4aa7859389998" },
  "tokenizer.json": { size: 2224119, gitSha1: "bc1f77d20440541dd073ebae6f6c401087c7d34e" },
  "tokenizer_config.json": { size: 775, gitSha1: "c9b2a711cbdd039529fa5a66f1965e13eb14451d" },
  "special_tokens_map.json": { size: 472, gitSha1: "2c2130b544c0c5a72d5d00da071ba130a9800fb2" },
  "onnx/text_model_quantized.onnx": {
    size: 64504507,
    sha256: "73baab855d406190da9faa498cfedf65f15cf309f4cc7385b7b032e6d08e5c3a",
  },
  "onnx/vision_model_quantized.onnx": {
    size: 89117001,
    sha256: "583fd1110a514667812fee7d684952aaf82a99b959760c8d7dca7e0ab9839299",
  },
};

const args = new Set(process.argv.slice(2));
const FORCE = args.has("--force");
const CHECK_ONLY = args.has("--check");

function formatBytes(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes;
  let unit = -1;
  do {
    value /= 1024;
    unit += 1;
  } while (value >= 1024 && unit < units.length - 1);
  return `${value.toFixed(1)} ${units[unit]}`;
}

async function sizeOf(path) {
  try {
    return (await stat(path)).size;
  } catch {
    return null;
  }
}

/** Digest of a file in the form its manifest entry records it. */
export async function digestOf(path, spec) {
  const hash = createHash(spec.sha256 ? "sha256" : "sha1");
  // A git blob id hashes a "blob <size>\0" header followed by the content.
  if (!spec.sha256) hash.update(`blob ${spec.size}\0`);
  await pipeline(createReadStream(path), hash);
  return hash.digest("hex");
}

/** True when the file at `path` matches its manifest entry exactly. */
export async function matchesManifest(path, spec) {
  if ((await sizeOf(path)) !== spec.size) return false;
  return (await digestOf(path, spec)) === (spec.sha256 ?? spec.gitSha1);
}

async function download(file, spec) {
  const expected = spec.size;
  const target = join(MODEL_DIR, ...file.split("/"));
  const partial = `${target}.part`;
  await mkdir(dirname(target), { recursive: true });

  if (!FORCE && (await matchesManifest(target, spec))) {
    console.log(`  = ${file} (${formatBytes(expected)}) already present and verified`);
    return expected;
  }
  if (FORCE) await rm(partial, { force: true });

  let offset = (await sizeOf(partial)) ?? 0;
  if (offset > expected) {
    await rm(partial, { force: true });
    offset = 0;
  }
  const url = `${ENDPOINT}/${MODEL_ID}/resolve/${encodeURIComponent(REVISION)}/${file}`;
  const headers = { "User-Agent": "CinaVault-model-fetch/1.0" };
  if (offset > 0) headers.Range = `bytes=${offset}-`;

  let response;
  try {
    response = await fetch(url, { headers, redirect: "follow" });
  } catch (error) {
    throw new Error(`network error fetching ${url}: ${error.cause?.message ?? error.message}`);
  }
  if (response.status === 416 && offset === expected) {
    return accept(file, spec, partial, target, false);
  }
  if (!response.ok || !response.body) {
    throw new Error(`HTTP ${response.status} ${response.statusText} for ${url}`);
  }
  const resumed = response.status === 206;
  if (!resumed) offset = 0; // server ignored the Range header: start over

  const total = expected;
  let received = offset;
  let lastLog = 0;
  const body = Readable.fromWeb(response.body);
  body.on("data", (chunk) => {
    received += chunk.length;
    const now = Date.now();
    if (process.stdout.isTTY && now - lastLog > 500) {
      lastLog = now;
      process.stdout.write(`\r  ↓ ${file} ${formatBytes(received)} / ${formatBytes(total)}   `);
    }
  });
  await pipeline(body, createWriteStream(partial, { flags: resumed ? "a" : "w" }));
  if (process.stdout.isTTY) process.stdout.write("\r");

  const size = await sizeOf(partial);
  if (size !== expected) {
    throw new Error(
      `${file}: size ${size} != expected ${expected}. Re-run to resume, or --force to restart.`,
    );
  }
  return accept(file, spec, partial, target, resumed);
}

/** Moves a complete download into place only if its digest matches; otherwise discards it. */
async function accept(file, spec, partial, target, resumed) {
  if (!(await matchesManifest(partial, spec))) {
    await rm(partial, { force: true });
    throw new Error(`${file}: digest does not match the pinned revision ${REVISION}; download discarded.`);
  }
  await rename(partial, target);
  console.log(`  + ${file} (${formatBytes(spec.size)}) verified${resumed ? " [resumed]" : ""}`);
  return spec.size;
}

async function check() {
  let ok = true;
  for (const [file, spec] of Object.entries(MODEL_FILES)) {
    const path = join(MODEL_DIR, ...file.split("/"));
    const size = await sizeOf(path);
    if (size === null || !(await matchesManifest(path, spec))) {
      ok = false;
      console.log(`  ✗ ${file}: ${size === null ? "missing" : "size or digest does not match the pinned revision"}`);
    }
  }
  console.log(ok ? "Local AI model files are present." : "Local AI model files are incomplete; run npm run fetch:ai-models.");
  return ok;
}

async function main() {
  console.log(`CinaVault local AI: ${MODEL_ID}@${REVISION} → ${MODEL_DIR}`);
  if (CHECK_ONLY) {
    process.exit((await check()) ? 0 : 1);
  }

  let modelBytes = 0;
  const failures = [];
  for (const [file, spec] of Object.entries(MODEL_FILES)) {
    try {
      modelBytes += await download(file, spec);
    } catch (error) {
      failures.push(`${file}: ${error.message}`);
      console.error(`  ✗ ${file}: ${error.message}`);
    }
  }
  if (failures.length > 0) {
    console.error(
      `\n${failures.length} file(s) failed. Partial downloads were kept and will resume on the next run.\n` +
        "The app still works without bundled weights: it downloads them from the Hugging Face CDN on first use.",
    );
    process.exit(1);
  }
  console.log(`Done: ${formatBytes(modelBytes)} of model files ready to bundle.`);
}

// Run only as a CLI, so tests can import the manifest and digest helpers.
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(`fetch-ai-models failed: ${error.stack ?? error}`);
    process.exit(1);
  });
}
