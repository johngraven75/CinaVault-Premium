import { invoke } from "@tauri-apps/api/core";
import { FULL_PLUGIN_REGISTRY, type PluginEntry } from "./pluginRegistry";
import {
  PGMA_PLUGIN_ID,
  PGMA_DEFAULT_CONFIG,
  pluginEngine,
} from "./pluginAdapter";

declare module "./pluginAdapter" {
  interface PluginAdapterEngine {
    initialize(): Promise<void>;
  }
}

type PluginBootConfig = Record<string, unknown>;

/** Built-in CinaVault native plugins that must be ready at startup. */
function getStartupPlugins(): PluginEntry[] {
  return FULL_PLUGIN_REGISTRY.filter(
    (plugin) => plugin.cinavaultNative && plugin.status === "active",
  );
}

function defaultConfigFor(plugin: PluginEntry): PluginBootConfig {
  if (plugin.id === PGMA_PLUGIN_ID) {
    return { ...PGMA_DEFAULT_CONFIG, enabled: true, installedAtBoot: true };
  }

  return {
    schemaVersion: 1,
    pluginId: plugin.id,
    name: plugin.name,
    version: plugin.version,
    platform: plugin.platforms[0] || "cinavault",
    enabled: true,
    installedAtBoot: true,
    configurable: plugin.configurable,
    category: plugin.category,
    status: "ready",
    source: "cinavault-default",
  };
}

function isValidConfig(raw: string | undefined): boolean {
  if (!raw || !raw.trim()) return false;
  try {
    const parsed = JSON.parse(raw);
    return Boolean(
      parsed && typeof parsed === "object" && !Array.isArray(parsed),
    );
  } catch {
    return false;
  }
}

async function installAndValidatePlugin(plugin: PluginEntry): Promise<void> {
  const installed = pluginEngine.getInstalledPlugin(plugin.id);
  const needsInstall = !installed;
  const config = defaultConfigFor(plugin);

  if (needsInstall) {
    await invoke("install_plugin", {
      pluginId: plugin.id,
      name: plugin.name,
      version: plugin.version,
      platforms: plugin.platforms,
      repoUrl: plugin.repo || "",
    });
  }

  const currentConfig = pluginEngine.getInstalledPlugin(plugin.id)?.configJson;
  if (needsInstall || !isValidConfig(currentConfig)) {
    await invoke("run_plugin", {
      pluginId: plugin.id,
      action: "configure",
      config: JSON.stringify(config),
    });
  }

  await invoke("run_plugin", {
    pluginId: plugin.id,
    action: "enable",
  });
}

async function initializePluginEngine(): Promise<void> {
  await pluginEngine.loadFromBackend();

  for (const plugin of getStartupPlugins()) {
    try {
      await installAndValidatePlugin(plugin);
    } catch (error) {
      console.warn(`Plugin boot validation skipped for ${plugin.id}:`, error);
    }
  }

  await pluginEngine.loadFromBackend();
}

if (typeof pluginEngine.initialize !== "function") {
  pluginEngine.initialize = initializePluginEngine;
}

export {};
