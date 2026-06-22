import type { PluginEntry, PluginPlatform } from "./pluginRegistry";

export type PluginBridgeKind =
  | "native"
  | "jellyfin-dll"
  | "emby-rest"
  | "plex-bundle"
  | "servarr-rest"
  | "process"
  | "webhook"
  | "metadata-bridge";

export interface UniversalPluginEntry extends PluginEntry {
  bridge: PluginBridgeKind;
  dependencies: string[];
  defaultConfig: Record<string, any>;
}

function plugin(
  id: string,
  name: string,
  description: string,
  platforms: PluginPlatform[],
  category: PluginEntry["category"],
  bridge: PluginBridgeKind,
  dependencies: string[],
  defaultConfig: Record<string, any> = {},
  extra: Partial<PluginEntry> = {},
): UniversalPluginEntry {
  return {
    id,
    name,
    description,
    version: extra.version || "latest",
    author: extra.author || "Community",
    platforms,
    category,
    status: extra.status || "available",
    icon: extra.icon || "🧩",
    repo: extra.repo,
    configurable: extra.configurable ?? true,
    premium: extra.premium ?? false,
    cinavaultNative: extra.cinavaultNative ?? true,
    tags: extra.tags || [category, bridge, ...platforms],
    bridge,
    dependencies,
    defaultConfig: {
      enabled: false,
      bridge,
      platforms,
      dependencies,
      runMode: "compatibility-adapter",
      saveConfigOnEnable: true,
      ...defaultConfig,
    },
  };
}

const serverConnectionDefaults = {
  serverUrl: "",
  apiKey: "",
  username: "",
  password: "",
  syncOnEnable: false,
};

const webhookDefaults = {
  webhookUrl: "",
  notifyOnPlayback: true,
  notifyOnNewMedia: true,
  notifyOnServerAlert: true,
};

export const UNIVERSAL_MEDIA_SERVER_PLUGINS: UniversalPluginEntry[] = [
  // Jellyfin/MS-C plugin bridge additions
  plugin("jf-ldap", "LDAP Authentication", "LDAP/Active Directory authentication provider.", ["jellyfin", "cinavault"], "security", "jellyfin-dll", ["Jellyfin.Plugin.LDAP.Auth.dll"], { ldapUrl: "", bindDn: "", searchBase: "" }, { icon: "🔐", author: "Jellyfin Team", tags: ["ldap", "auth", "security"] }),
  plugin("jf-sso", "SSO Authentication", "Single sign-on authentication bridge for OAuth/OIDC/SAML providers.", ["jellyfin", "cinavault"], "security", "jellyfin-dll", ["oidc-client", "oauth-callback"], { issuerUrl: "", clientId: "", clientSecret: "", callbackPort: 19284 }, { icon: "🪪", tags: ["sso", "oidc", "oauth"] }),
  plugin("jf-intros", "Intros", "Cinema intro/pre-roll playback before selected media.", ["jellyfin", "cinavault"], "playback", "jellyfin-dll", ["ffprobe"], { introFolder: "", playBeforeMovies: true, playBeforeEpisodes: false }, { icon: "🎞️", tags: ["intro", "pre-roll", "cinema"] }),
  plugin("jf-skin-manager", "Skin Manager", "Theme and skin management for server web clients.", ["jellyfin", "cinavault"], "themes", "jellyfin-dll", [], { selectedSkin: "default", customCss: "" }, { icon: "🎨", tags: ["theme", "skin", "css"] }),
  plugin("jf-anime", "Anime Metadata Bundle", "Anime metadata compatibility bundle: AniDB, AniList, Kitsu, AniSearch, MAL aliases.", ["jellyfin", "cinavault"], "metadata", "metadata-bridge", ["reqwest", "serde_json"], { providers: ["anidb", "anilist", "kitsu", "mal"], preferAbsoluteOrdering: true }, { icon: "🎌", tags: ["anime", "metadata"] }),

  // Emby/MS-B plugin bridge additions
  plugin("em-iptv", "IPTV", "IPTV channel and M3U ingestion bridge.", ["emby", "cinavault"], "live_tv", "emby-rest", ["m3u-parser", "epg-parser"], { ...serverConnectionDefaults, m3uUrl: "", epgUrl: "" }, { icon: "📡", tags: ["iptv", "m3u", "epg"] }),
  plugin("em-ldap", "LDAP for MS-B", "LDAP authentication bridge for MS-B-compatible deployments.", ["emby", "cinavault"], "security", "emby-rest", ["ldap-client"], { ldapUrl: "", bindDn: "", searchBase: "" }, { icon: "🔐", tags: ["ldap", "auth"] }),
  plugin("em-backup", "Server Configuration Backup", "Backup and restore server plugin/user/library configuration.", ["emby", "cinavault"], "management", "emby-rest", ["zip"], { backupFolder: "", includePluginConfigs: true, includeUsers: false }, { icon: "💾", tags: ["backup", "restore"] }),
  plugin("em-statistics", "Statistics", "Server/library statistics dashboard bridge.", ["emby", "cinavault"], "stats", "emby-rest", ["sqlite"], { retentionDays: 365, collectPlayback: true, collectLibraryStats: true }, { icon: "📊", tags: ["stats", "analytics"] }),

  // Plex/MS-A plugins and tools
  plugin("px-webtools-ng", "WebTools-NG", "Plex WebTools-NG compatibility launcher and utility bridge.", ["plex", "cinavault"], "utilities", "process", ["node", "shell-open"], { webtoolsPath: "", launchMode: "external" }, { icon: "🧰", repo: "https://github.com/WebTools-NG/WebTools-NG", tags: ["plex", "webtools"] }),
  plugin("px-kometa", "Kometa", "Collections, overlays, playlists, and metadata automation for Plex/Jellyfin/Emby libraries.", ["plex", "jellyfin", "emby", "cinavault"], "management", "process", ["python3", "kometa", "yaml"], { configPath: "", runOnSchedule: false, schedule: "daily", dryRun: true }, { icon: "🧱", repo: "https://github.com/Kometa-Team/Kometa", tags: ["collections", "overlays", "automation"] }),
  plugin("px-tautulli", "Tautulli", "Plex analytics, history, and notification bridge.", ["plex", "cinavault"], "stats", "webhook", ["tautulli-api"], { ...serverConnectionDefaults, tautulliUrl: "", tautulliApiKey: "" }, { icon: "📈", repo: "https://github.com/Tautulli/Tautulli", tags: ["plex", "stats", "history"] }),
  plugin("px-pastatool", "PastaTool", "Bulk audio/subtitle stream selector for Plex libraries.", ["plex", "cinavault"], "utilities", "process", ["python3", "plexapi"], { plexUrl: "", plexToken: "", preferredAudio: "", preferredSubtitles: "" }, { icon: "🍝", tags: ["plex", "subtitles", "audio"] }),
  plugin("px-subzero", "Sub-Zero", "Legacy Plex subtitle provider bridge for migration/compatibility.", ["plex", "cinavault"], "subtitles", "plex-bundle", ["Plex.bundle", "OpenSubtitles API"], { languages: ["en"], migrateToOpenSubtitles: true }, { icon: "❄️", tags: ["plex", "subtitles"] }),
  plugin("px-lambda-agents", "Lambda/ZeroQI Agents", "Legacy Plex scanner/agent compatibility bundle for anime and Hama-style libraries.", ["plex", "cinavault"], "metadata", "plex-bundle", ["Plex.bundle", "python2-compat"], { scannerMode: "compat", writeSidecars: false }, { icon: "λ", tags: ["plex", "anime", "scanner"] }),

  // Servarr, request, subtitle, downloader, and notification ecosystem
  plugin("svc-sonarr", "Sonarr", "TV show automation integration.", ["cinavault", "plex", "jellyfin", "emby"], "management", "servarr-rest", ["Sonarr API"], { ...serverConnectionDefaults, rootFolder: "", qualityProfileId: "" }, { icon: "📺", repo: "https://github.com/Sonarr/Sonarr", tags: ["sonarr", "tv", "automation"] }),
  plugin("svc-radarr", "Radarr", "Movie automation integration.", ["cinavault", "plex", "jellyfin", "emby"], "management", "servarr-rest", ["Radarr API"], { ...serverConnectionDefaults, rootFolder: "", qualityProfileId: "" }, { icon: "🎬", repo: "https://github.com/Radarr/Radarr", tags: ["radarr", "movies", "automation"] }),
  plugin("svc-lidarr", "Lidarr", "Music automation integration.", ["cinavault", "plex", "jellyfin", "emby"], "management", "servarr-rest", ["Lidarr API"], { ...serverConnectionDefaults, rootFolder: "", qualityProfileId: "" }, { icon: "🎵", repo: "https://github.com/Lidarr/Lidarr", tags: ["lidarr", "music"] }),
  plugin("svc-readarr", "Readarr", "Book/audiobook automation integration.", ["cinavault", "plex", "jellyfin", "emby"], "management", "servarr-rest", ["Readarr API"], { ...serverConnectionDefaults, rootFolder: "", qualityProfileId: "" }, { icon: "📚", repo: "https://github.com/Readarr/Readarr", tags: ["readarr", "books"] }),
  plugin("svc-prowlarr", "Prowlarr", "Universal indexer manager integration.", ["cinavault", "plex", "jellyfin", "emby"], "management", "servarr-rest", ["Prowlarr API"], { ...serverConnectionDefaults, syncApps: true }, { icon: "🔎", repo: "https://github.com/Prowlarr/Prowlarr", tags: ["prowlarr", "indexers"] }),
  plugin("svc-bazarr", "Bazarr", "Subtitle automation integration.", ["cinavault", "plex", "jellyfin", "emby"], "subtitles", "servarr-rest", ["Bazarr API"], { ...serverConnectionDefaults, languages: ["en"], autoDownload: true }, { icon: "💬", repo: "https://github.com/morpheus65535/bazarr", tags: ["bazarr", "subtitles"] }),
  plugin("svc-jellyseerr", "Jellyseerr", "Request management bridge for Jellyfin/Emby libraries.", ["jellyfin", "emby", "cinavault"], "management", "servarr-rest", ["Jellyseerr API"], { ...serverConnectionDefaults, autoApprove: false }, { icon: "🧞", repo: "https://github.com/Fallenbagel/jellyseerr", tags: ["requests", "jellyfin", "emby"] }),
  plugin("svc-overseerr", "Overseerr", "Request management bridge for Plex libraries.", ["plex", "cinavault"], "management", "servarr-rest", ["Overseerr API"], { ...serverConnectionDefaults, autoApprove: false }, { icon: "🛰️", repo: "https://github.com/sct/overseerr", tags: ["requests", "plex"] }),
  plugin("svc-ombi", "Ombi", "Cross-server request portal bridge.", ["plex", "jellyfin", "emby", "cinavault"], "management", "servarr-rest", ["Ombi API"], { ...serverConnectionDefaults, autoApprove: false }, { icon: "📬", repo: "https://github.com/Ombi-app/Ombi", tags: ["requests", "portal"] }),

  // Notification bridges
  plugin("ntfy", "ntfy", "Push notification bridge.", ["cinavault", "plex", "jellyfin", "emby"], "notifications", "webhook", ["HTTP webhooks"], { ...webhookDefaults, topic: "cinavault" }, { icon: "📣", tags: ["ntfy", "notifications"] }),
  plugin("gotify", "Gotify", "Self-hosted push notification bridge.", ["cinavault", "plex", "jellyfin", "emby"], "notifications", "webhook", ["Gotify API"], { ...webhookDefaults, token: "" }, { icon: "🔔", tags: ["gotify", "notifications"] }),
  plugin("discord-webhook", "Discord Webhook", "Discord rich notification bridge.", ["cinavault", "plex", "jellyfin", "emby"], "notifications", "webhook", ["Discord webhook"], { ...webhookDefaults, embedColor: 60159 }, { icon: "💬", tags: ["discord", "webhook"] }),
  plugin("slack-webhook", "Slack Webhook", "Slack notification bridge.", ["cinavault", "plex", "jellyfin", "emby"], "notifications", "webhook", ["Slack webhook"], { ...webhookDefaults, channel: "" }, { icon: "#️⃣", tags: ["slack", "webhook"] }),
];

const DEFAULT_CONFIG_BY_ID = new Map<string, Record<string, any>>(
  UNIVERSAL_MEDIA_SERVER_PLUGINS.map((entry) => [entry.id, entry.defaultConfig]),
);

export function getUniversalDefaultPluginConfig(pluginId: string): Record<string, any> {
  const config = DEFAULT_CONFIG_BY_ID.get(pluginId);
  return config ? { ...config } : {};
}

export function mergeUniquePlugins(base: PluginEntry[], extras: PluginEntry[] = UNIVERSAL_MEDIA_SERVER_PLUGINS): PluginEntry[] {
  const seen = new Set<string>();
  const merged: PluginEntry[] = [];
  for (const plugin of [...base, ...extras]) {
    if (seen.has(plugin.id)) continue;
    seen.add(plugin.id);
    merged.push(plugin);
  }
  return merged;
}
