// CinaVault Premium — Universal Plugin Compatibility Adapter
// Provides runtime bridge to load, configure, and execute MS-C / MS-B / MS-A plugins.

import { invoke } from "@tauri-apps/api/core";
import type { PluginEntry, PluginPlatform } from "./pluginRegistry";
import { getUniversalDefaultPluginConfig } from "./universalMediaServerPlugins";

export const PGMA_PLUGIN_ID = "px-pgma-modernized";

export const PGMA_DEFAULT_CONFIG = {
  plexPluginPath: "",
  sourceZipUrl: "https://github.com/CodyBerenson/PGMA-Modernized/archive/refs/heads/master.zip",
  defaultTarget: "cinavault-staging",
  notes: "Leave plexPluginPath blank to deploy into CinaVault's local Plex plugin staging folder. Set it only when you want to deploy directly into a real Plex Plug-ins folder.",
  requiresPlexRestart: true,
  nativeToolchain: "native-rust-pgma-bridge",
  metadataSources: ["nfo", "localArtwork"],
  downloadArtwork: true,
  overwriteExistingMetadata: false,
  limit: 5000,
  autoDeployBundlesOnInstall: true,
  autoRefreshLibraryAfterDeploy: false,
};

export interface AdapterConfig {
  platform: PluginPlatform;
  basePath: string;
  apiBase?: string;
  apiKey?: string;
}

export interface InstalledPlugin {
  id: string;
  name: string;
  platform: PluginPlatform;
  version: string;
  installPath: string;
  configJson: string;
  enabled: boolean;
  lastRun?: string;
}

const JELLYFIN_DLL_MAP: Record<string, string> = {
  "jf-opensubtitles": "Jellyfin.Plugin.OpenSubtitles.dll",
  "jf-trakt": "Jellyfin.Plugin.Trakt.dll",
  "jf-fanart": "Jellyfin.Plugin.Fanart.dll",
  "jf-tvdb": "Jellyfin.Plugin.TheTvdb.dll",
  "jf-anidb": "Jellyfin.Plugin.AniDB.dll",
  "jf-anilist": "Jellyfin.Plugin.AniList.dll",
  "jf-kitsu": "Jellyfin.Plugin.Kitsu.dll",
  "jf-webhook": "Jellyfin.Plugin.Webhook.dll",
  "jf-ldap": "Jellyfin.Plugin.LDAP.Auth.dll",
  "jf-reports": "Jellyfin.Plugin.Reports.dll",
  "jf-playback-reporting": "Jellyfin.Plugin.PlaybackReporting.dll",
  "jf-tmdb-boxsets": "Jellyfin.Plugin.TMDbBoxSets.dll",
  "jf-bookshelf": "Jellyfin.Plugin.Bookshelf.dll",
  "jf-kodi-sync": "Jellyfin.Plugin.KodiSyncQueue.dll",
  "jf-dlna": "Jellyfin.Plugin.Dlna.dll",
  "jf-chapter-segments": "Jellyfin.Plugin.ChapterSegments.dll",
};

function shouldLogInvokeFailure(): boolean {
  return typeof window !== "undefined" && Boolean((window as any).__TAURI_INTERNALS__);
}

function normalizePlatform(platform: unknown): PluginPlatform {
  return ["jellyfin", "emby", "plex", "cinavault"].includes(String(platform))
    ? (platform as PluginPlatform)
    : "cinavault";
}

function genericDefaultConfig(pluginId: string): Record<string, unknown> {
  return {
    schemaVersion: 1,
    pluginId,
    enabled: false,
    configured: true,
    readyWhenEnabled: true,
    saveConfigOnEnable: true,
    runMode: "compatibility-adapter",
  };
}

function defaultConfigForPlugin(pluginId: string): Record<string, unknown> {
  if (pluginId === PGMA_PLUGIN_ID) return { ...PGMA_DEFAULT_CONFIG };
  const universalConfig = getUniversalDefaultPluginConfig(pluginId);
  return Object.keys(universalConfig).length > 0
    ? { ...universalConfig }
    : genericDefaultConfig(pluginId);
}

function isPgmaDeployAction(action: string): boolean {
  return ["deploy", "install", "update", "upgrade"].includes(action);
}

function isPgmaRefreshAction(action: string): boolean {
  return ["start", "run", "refresh", "refresh_library", "refreshLibrary"].includes(action);
}

export class PluginAdapterEngine {
  private adapters: Map<PluginPlatform, AdapterConfig> = new Map();
  private installed: Map<string, InstalledPlugin> = new Map();

  constructor() {
    this.adapters.set("jellyfin", { platform: "jellyfin", basePath: "%APPDATA%/CinaVault/plugins/jellyfin" });
    this.adapters.set("emby", { platform: "emby", basePath: "%APPDATA%/CinaVault/plugins/emby" });
    this.adapters.set("plex", { platform: "plex", basePath: "%APPDATA%/CinaVault/plugins/plex/Plug-ins" });
    this.adapters.set("cinavault", { platform: "cinavault", basePath: "%APPDATA%/CinaVault/plugins/native" });
  }

  async installPlugin(plugin: PluginEntry): Promise<boolean> {
    const defaultConfig = defaultConfigForPlugin(plugin.id);
    if (this.installed.has(plugin.id)) {
      await this.setPluginEnabled(plugin.id, true);
      await this.setPluginConfig(plugin.id, { ...defaultConfig, ...this.getPluginConfig(plugin.id), enabled: true });
      return true;
    }

    try {
      await invoke("install_plugin", {
        pluginId: plugin.id,
        name: plugin.name,
        version: plugin.version,
        platforms: plugin.platforms,
        repoUrl: plugin.repo || "",
      });

      let deployResult: any = null;
      if (plugin.id === PGMA_PLUGIN_ID) {
        await invoke("run_plugin", {
          pluginId: plugin.id,
          action: "configure",
          config: JSON.stringify(defaultConfig),
        });
        if ((defaultConfig as typeof PGMA_DEFAULT_CONFIG).autoDeployBundlesOnInstall) {
          deployResult = await invoke("run_plugin", {
            pluginId: plugin.id,
            action: "deploy",
            config: JSON.stringify(defaultConfig),
          });
        }
      } else {
        await invoke("run_plugin", {
          pluginId: plugin.id,
          action: "configure",
          config: JSON.stringify(defaultConfig),
        });
      }

      const installPath = plugin.id === PGMA_PLUGIN_ID && deployResult?.targetPath
        ? deployResult.targetPath
        : this.resolveInstallPath(plugin);
      this.installed.set(plugin.id, {
        id: plugin.id,
        name: plugin.name,
        platform: plugin.platforms[0] || "cinavault",
        version: plugin.version,
        installPath,
        configJson: JSON.stringify({
          ...defaultConfig,
          enabled: true,
          lastDeployTarget: deployResult?.targetPath,
          deployedBundles: deployResult?.bundles,
        }),
        enabled: true,
        lastRun: new Date().toISOString(),
      });
      return true;
    } catch (err) {
      if (shouldLogInvokeFailure()) console.warn(`Plugin install failed: ${plugin.id}`, err);
      this.installed.set(plugin.id, {
        id: plugin.id,
        name: plugin.name,
        platform: plugin.platforms[0] || "cinavault",
        version: plugin.version,
        installPath: plugin.id === PGMA_PLUGIN_ID
          ? "%APPDATA%/CinaVault/plugins/plex/Plug-ins"
          : `plugins/${plugin.platforms[0] || "cinavault"}/${plugin.id}`,
        configJson: JSON.stringify({ ...defaultConfig, enabled: true }),
        enabled: true,
      });
      return true;
    }
  }

  bootstrapCatalog(plugins: PluginEntry[]): number {
    void plugins;
    return 0;
  }

  async uninstallPlugin(pluginId: string): Promise<boolean> {
    if (pluginId === PGMA_PLUGIN_ID) {
      await this.setPluginEnabled(pluginId, true);
      return true;
    }
    try { await invoke("uninstall_plugin", { pluginId }); } catch {}
    this.installed.delete(pluginId);
    return true;
  }

  async runPlugin(pluginId: string, action = "start"): Promise<unknown> {
    const configObject = { ...defaultConfigForPlugin(pluginId), ...this.getPluginConfig(pluginId) };
    const config = JSON.stringify(configObject);
    try {
      let result: unknown;
      if (pluginId === PGMA_PLUGIN_ID && isPgmaRefreshAction(action)) {
        result = await invoke("refresh_pgma_library", { config });
      } else if (pluginId === PGMA_PLUGIN_ID && isPgmaDeployAction(action)) {
        result = await invoke("run_plugin", { pluginId, action: "deploy", config });
      } else {
        result = await invoke("run_plugin", { pluginId, action, config });
      }

      if (pluginId === PGMA_PLUGIN_ID && result && typeof result === "object") {
        const response = result as Record<string, any>;
        const current = this.installed.get(pluginId);
        const nextConfig = {
          ...PGMA_DEFAULT_CONFIG,
          ...this.getPluginConfig(pluginId),
          lastDeployTarget: response.targetPath,
          deployedBundles: response.bundles,
          lastRefreshStats: response.scanned !== undefined ? {
            scanned: response.scanned,
            matched: response.matched,
            updated: response.updated,
            artworkDownloaded: response.artworkDownloaded,
            skipped: response.skipped,
            errors: response.errors,
            message: response.message,
          } : undefined,
          requiresPlexRestart: response.requiresPlexRestart ?? true,
        };
        if (current) {
          this.installed.set(pluginId, {
            ...current,
            installPath: response.targetPath || current.installPath,
            configJson: JSON.stringify(nextConfig),
            enabled: true,
            lastRun: new Date().toISOString(),
          });
        }
      }
      return result;
    } catch (err) {
      if (shouldLogInvokeFailure()) console.warn(`Plugin run failed: ${pluginId}`, err);
      return { success: false, error: String(err) };
    }
  }

  getPluginConfig(pluginId: string): Record<string, unknown> {
    const plugin = this.installed.get(pluginId);
    if (!plugin) return defaultConfigForPlugin(pluginId);
    try {
      return { ...defaultConfigForPlugin(pluginId), ...JSON.parse(plugin.configJson) };
    } catch {
      return defaultConfigForPlugin(pluginId);
    }
  }

  async setPluginConfig(pluginId: string, config: Record<string, unknown>): Promise<void> {
    const nextConfig = { ...defaultConfigForPlugin(pluginId), ...config };
    const plugin = this.installed.get(pluginId);
    if (plugin) {
      this.installed.set(pluginId, { ...plugin, configJson: JSON.stringify(nextConfig) });
    }
    try {
      await invoke("run_plugin", { pluginId, action: "configure", config: JSON.stringify(nextConfig) });
    } catch {}
  }

  async setPluginEnabled(pluginId: string, enabled: boolean): Promise<void> {
    const plugin = this.installed.get(pluginId);
    const nextConfig = { ...this.getPluginConfig(pluginId), enabled };
    if (plugin) {
      this.installed.set(pluginId, { ...plugin, enabled, configJson: JSON.stringify(nextConfig) });
    }
    try {
      await invoke("run_plugin", {
        pluginId,
        action: enabled ? "enable" : "disable",
        config: JSON.stringify(nextConfig),
      });
    } catch {}
  }

  checkCompatibility(plugin: PluginEntry): { compatible: boolean; reason: string } {
    if (plugin.id === PGMA_PLUGIN_ID) return { compatible: true, reason: "Native bundle deployer + CinaVault metadata bridge" };
    const universalConfig = getUniversalDefaultPluginConfig(plugin.id);
    if (Object.keys(universalConfig).length > 0) {
      const bridge = String(universalConfig.bridge || "compatibility-adapter");
      return { compatible: true, reason: `Universal bridge: ${bridge}` };
    }
    if (plugin.cinavaultNative) return { compatible: true, reason: "CinaVault native adapter available" };
    if (plugin.platforms.includes("jellyfin")) {
      const dll = JELLYFIN_DLL_MAP[plugin.id];
      return { compatible: true, reason: dll ? `MS-C DLL bridge: ${dll}` : "MS-C API-compatible adapter" };
    }
    if (plugin.platforms.includes("emby")) return { compatible: true, reason: "MS-B REST API adapter" };
    if (plugin.platforms.includes("plex")) return { compatible: true, reason: "MS-A tool bridge (CLI/Python)" };
    return { compatible: false, reason: "No adapter available" };
  }

  private resolveInstallPath(plugin: PluginEntry): string {
    if (plugin.id === PGMA_PLUGIN_ID) return "%APPDATA%/CinaVault/plugins/plex/Plug-ins";
    const platform = plugin.platforms[0] || "cinavault";
    const adapter = this.adapters.get(platform);
    return `${adapter?.basePath || "plugins"}/${plugin.id}`;
  }

  getInstalled(): InstalledPlugin[] {
    return Array.from(this.installed.values());
  }

  getInstalledPlugin(pluginId: string): InstalledPlugin | undefined {
    return this.installed.get(pluginId);
  }

  isInstalled(pluginId: string): boolean {
    return this.installed.has(pluginId);
  }

  async loadFromBackend(): Promise<void> {
    try {
      const plugins = await invoke<any[]>("get_installed_plugins");
      const loaded = new Map<string, InstalledPlugin>();
      for (const plugin of plugins) {
        const id = String(plugin.id || plugin.pluginId || plugin.name || "");
        if (!id) continue;
        loaded.set(id, {
          id,
          name: plugin.name || id,
          platform: normalizePlatform(plugin.platform),
          version: plugin.version || "1.0.0",
          installPath: plugin.installPath || "",
          configJson: plugin.configJson || JSON.stringify(defaultConfigForPlugin(id)),
          enabled: plugin.enabled !== false,
        });
      }
      this.installed = loaded;
    } catch {
      // No backend — running in browser/dev mode.
    }
  }
}

export const pluginEngine = new PluginAdapterEngine();
