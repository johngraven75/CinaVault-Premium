# Local AI vision (poster and film identification)

CinaVault identifies films and checks posters with a free AI model that runs inside the app. It needs no API key, no Hugging Face token, no account and no paid service, and it works on first launch with no setup.

## Model

- **Model:** `Xenova/clip-vit-base-patch32`. This is OpenAI CLIP ViT-B/32 exported to ONNX with 8-bit quantization. MIT licensed.
- **Files used:** `onnx/vision_model_quantized.onnx` (85 MB), `onnx/text_model_quantized.onnx` (61.5 MB), plus tokenizer and config JSON (about 2.2 MB). The total is about **149 MB**.
- **Runtime:** [Transformers.js](https://github.com/huggingface/transformers.js) (`@huggingface/transformers`, pinned to an exact version in `package.json`) running on ONNX Runtime Web in the WebView. It uses WebGPU when the GPU adapter supports it. Otherwise it uses WASM (single-threaded, because the WebView is not cross-origin isolated). If WebGPU fails to initialize, it falls back to WASM automatically.

## Where it is stored and how it is bundled

1. `npm run fetch:ai-models` (`scripts/fetch-ai-models.mjs`) downloads the files into `public/models/Xenova/clip-vit-base-patch32/`.
   - Files come from one pinned Hub commit, and each file is checked against a digest recorded in the script (SHA-256 for the ONNX weights, the git blob id for the JSON files). A file that doesn't match is discarded and the build fails. The runtime download fallback pins the same commit.
   - It is idempotent: files that already match their digest are skipped.
   - It resumes interrupted downloads from `*.part` files using HTTP Range requests. `--force` re-downloads everything.
   - `npm run check:ai-models` only checks the files and exits 1 if any are missing.
   - `HF_ENDPOINT` selects a Hub mirror; its files must still match the pinned digests.
   - To move to a newer model revision, update `REVISION` and `MODEL_FILES` in the script and `VISION_MODEL_REVISION` in `src/services/localVision.ts` together.
2. `vite build` copies `public/` into `dist/`, and Tauri packs `dist/` into the installer. At runtime the app loads the model from `/models/...` on its own origin.
3. Vite emits the ONNX Runtime WASM binary (about 26 MB) from `node_modules/onnxruntime-web` as a hashed asset in `dist/assets/`. Its JS loader is inlined in the lazily loaded Transformers.js chunk (about 584 kB, 170 kB gzip). Neither one is fetched from a CDN in production builds.
4. `public/models/` is in `.gitignore`. Model weights are never committed. Release CI must run `npm run fetch:ai-models` before `tauri build` to ship an installer that works offline.

## Offline behavior

| Situation | What happens |
|---|---|
| Installer built with the bundled model | Fully offline from first launch. |
| Dev build or installer without bundled weights | The model downloads once from `huggingface.co` (public, no token needed). The WebView Cache API keeps it, so later launches are offline. |
| No network and no bundled weights | Vision features report `state: "error"` through `getVisionStatus()`. The rest of the app is unaffected, and `warmUpVision()` never throws. |
| `vite` dev server | ONNX Runtime WASM loads from the Transformers.js default CDN (jsDelivr). This is needed because Vite's dependency pre-bundling breaks ORT's `new URL(..., import.meta.url)`. |

## Code map

- `src/services/localVision.ts` contains the loader and embeddings. It loads the model once even when several callers ask at the same time (single-flight), and caches image embeddings per `src` in an LRU cache.
  - `warmUpVision()` is called at boot. It does not block startup and never throws.
  - Other exports: `isVisionReady()`, `embedImage(src)`, `embedText(text)`, `embedTexts(texts)`, `classifyImage(src, labels)`, `getVisionStatus()`, `onVisionStatus(fn)`.
- `src/services/visionMath.ts` holds pure math with no model dependency: `cosineSimilarity`, `normalize`, `softmax`, `rankCandidates`, `decideMatch`.
- `src/services/posterIdentification.ts`:
  - `verifyPoster(item)` checks whether a poster belongs to the title, using CLIP zero-shot classification against generic negatives.
  - `pickBestPoster(reference, urls)` picks the candidate poster closest to a reference frame or poster.
  - `identifyFromCandidates(item, candidates)` ranks the results of the `search_metadata` command by title, year and poster similarity. It returns `autoApply: true` only when the best score is at least 0.82 and leads the runner-up by at least 0.08.
  - `searchAndIdentify(item, provider)` calls `search_metadata` and then `identifyFromCandidates`.
  - All scoring rules are pure functions and are tested in `tests/localVision.test.mjs` (`npm run test:local-vision`).

## Platform requirements

- The Tauri CSP `script-src` must allow `'wasm-unsafe-eval'`. Otherwise the WebView refuses to compile ONNX Runtime's WebAssembly.
- For the remote fallback, `connect-src` must allow `https://huggingface.co`, `https://*.hf.co` and `https://cdn-lfs.huggingface.co`. The current `https:` wildcard already covers them.
- To embed local poster files on macOS and Linux, `connect-src` must also allow `asset:` and `http://asset.localhost`. On Windows, `http:` already covers `http://asset.localhost`.
- `onnxruntime-node` is an install-time dependency of Transformers.js, but the app never uses it. On Linux x64 its postinstall step downloads CUDA libraries from NuGet. Set `ONNXRUNTIME_NODE_INSTALL=skip` (or add `onnxruntime-node-install=skip` to `.npmrc`) on Linux CI to skip that download.

## At boot

`src/services/aiPosterAutopilot.ts` starts 8 seconds after launch. It warms the model and then checks unverified library items that already have a poster:

- A poster that matches its title marks the item verified.
- A poster that is clearly art for a different title is counted as mismatched.

It never deletes or rewrites anything. To turn it off, set `ai_poster_autoverify` to `false`. The app CSP allows `'wasm-unsafe-eval'` so ONNX Runtime can compile its WASM.
