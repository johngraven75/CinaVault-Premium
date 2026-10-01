import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

const read = (path) => fs.readFileSync(path, "utf8");

test("store-safe edition is a build mode of the single Premium codebase", () => {
  assert.match(read(".env.store-safe"), /VITE_STORE_SAFE=true/);
  const edition = read("src/config/edition.ts");
  assert.match(edition, /IS_STORE_SAFE = import\.meta\.env\.VITE_STORE_SAFE === "true"/);
  for (const fn of ["filterStoreSafeMedia", "filterStoreSafeSources", "filterStoreSafeProviders"]) {
    assert.match(edition, new RegExp(`export function ${fn}`));
    assert.match(read("src/store/appStore.ts"), new RegExp(fn));
  }

  const pkg = JSON.parse(read("package.json"));
  assert.equal(pkg.scripts["build:ms-v1"], "tsc && vite build --mode store-safe");
  const msv1 = JSON.parse(read("src-tauri/tauri.msv1.conf.json"));
  assert.equal(msv1.build.beforeBuildCommand, "npm run build:ms-v1");
  assert.deepEqual(msv1.bundle.resources, ["../plugins/configs/store-safe/*.json"]);
  for (const file of fs.readdirSync("plugins/configs/store-safe")) {
    assert.doesNotMatch(read(`plugins/configs/store-safe/${file}`), /adult|porn|stash/i, file);
  }
});

test("adult-only surfaces are gated out of the store-safe edition", () => {
  const gated = {
    "src/components/tabs/MediaSourcesTab.tsx": /!IS_STORE_SAFE && <option value="adult">/,
    "src/components/tabs/PluginsTab.tsx": /IS_STORE_SAFE\s*\?\s*FULL_PLUGIN_REGISTRY\.filter\(\(plugin\) => !plugin\.tags\.includes\("adult"\)\)/,
    "src/components/tabs/HomeTab.tsx": /!IS_STORE_SAFE && selectedMedia\.media_type === "adult"/,
    "src/components/kodi/KodiHomeLayout.tsx": /!IS_STORE_SAFE \? \[\{ id: "adult"/,
    "src/components/tabs/AIDiagnosticsTab.tsx": /!IS_STORE_SAFE \|\| action\.progressTask !== "adult_metadata_gather"/,
  };
  for (const [file, pattern] of Object.entries(gated)) {
    assert.match(read(file), pattern, file);
  }
});
