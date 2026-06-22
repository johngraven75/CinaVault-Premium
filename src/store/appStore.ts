// CinaVault Premium — Global State Store (Zustand) with Persistence
import { create } from "zustand";
import { ADULT_METADATA_PROVIDERS } from "../data/adultMetadataProviders";
import { sanitizeMetadataProviders } from "../utils/pluginUiSafety";

export type TabId =
  | "home" | "sources" | "downloads" | "livetv" | "server"
  | "security" | "remote" | "advanced" | "cloud" | "plugins" | "ai" | "settings";

export interface MediaItem {
  id?: number;
  title: string;
  file_path: string;
  media_type: string;
  year?: number;
  rating?: number;
  overview?: string;
  poster_path?: string;
  backdrop_path?: string;
  genre?: string;
  duration?: number;
  file_size?: number;
  resolution?: string;
  codec?: string;
  verified: boolean;
  watched: boolean;
  favorite: boolean;
  date_added: string;
  last_played?: string;
  tmdb_id?: string;
  imdb_id?: string;
  source_id?: number;
}

export interface MediaSource {
  id?: number;
  path: string;
  source_type: string;
  name: string;
  enabled: boolean;
  last_scanned?: string;
  item_count: number;
}

export interface MetadataProvider {
  id: string;
  name: string;
  category: string;
  enabled: boolean;
}

export type TaskFrequency = "manual" | "on_scan" | "daily" | "weekly" | "on_import" | "never";

export interface ScheduledTaskConfig {
  thumbnails: TaskFrequency;
  chapter_images: TaskFrequency;
  metadata_check: TaskFrequency;
  match_unmatch: TaskFrequency;
}

export interface LibraryEnrichmentResult {
  type: "library_enrichment";
  status: string;
  mode: string;
  items_scanned: number;
  metadata_items_enriched: number;
  metadata_fields_updated: number;
  metadata_updated?: number;
  titles_improved: number;
  items_reclassified_as_adult: number;
  files_renamed: number;
  rename_collisions_skipped: number;
  rename_failures: number;
  low_confidence_metadata_only: number;
  skipped_missing_files: number;
  skipped_non_video_items: number;
  provider_errors: string[];
}

export type CloudServiceStatus = "connected" | "disconnected" | "connecting" | "error";

export interface CloudServiceState {
  id: string;
  status: CloudServiceStatus;
  account?: string;
  syncPath?: string;
  lastSync?: string;
}

export interface AppState {
  activeTab: TabId;
  sidebarCollapsed: boolean;
  setActiveTab: (tab: TabId) => void;
  toggleSidebar: () => void;

  currentTheme: string;
  setTheme: (theme: string) => void;

  mediaItems: MediaItem[];
  setMediaItems: (items: MediaItem[]) => void;
  selectedMedia: MediaItem | null;
  setSelectedMedia: (item: MediaItem | null) => void;
  libraryView: "list" | "card";
  setLibraryView: (view: "list" | "card") => void;
  searchQuery: string;
  setSearchQuery: (q: string) => void;

  sources: MediaSource[];
  setSources: (sources: MediaSource[]) => void;

  scanning: boolean;
  scanProgress: { total: number; current: number };
  setScanning: (s: boolean) => void;
  setScanProgress: (p: { total: number; current: number }) => void;

  serverRunning: boolean;
  serverType: string;
  serverUrl: string;
  setServerStatus: (running: boolean, type_: string, url: string) => void;

  downloading: boolean;
  setDownloading: (d: boolean) => void;

  vpnConnected: boolean;
  vpnLocation: string;
  setVpnStatus: (connected: boolean, location: string) => void;

  aiProcessing: boolean;
  aiResult: unknown;
  setAiProcessing: (p: boolean) => void;
  setAiResult: (r: unknown) => void;

  settings: Record<string, string>;
  setSettings: (s: Record<string, string>) => void;
  setSetting: (key: string, value: string) => void;

  featureSettings: Record<string, { enabled: boolean; config: unknown }>;
  setFeatureSettings: (fs: Record<string, { enabled: boolean; config: unknown }>) => void;
  toggleFeature: (key: string) => void;

  metadataProviders: MetadataProvider[];
  setMetadataProviders: (p: MetadataProvider[]) => void;
  toggleMetadataProvider: (id: string) => void;
  enableAllProviders: (category?: string) => void;
  disableAllProviders: (category?: string) => void;

  scheduledTasks: ScheduledTaskConfig;
  setScheduledTasks: (t: ScheduledTaskConfig) => void;
  setTaskFrequency: (task: keyof ScheduledTaskConfig, freq: TaskFrequency) => void;

  cloudServices: Record<string, CloudServiceState>;
  setCloudService: (id: string, state: Partial<CloudServiceState>) => void;

  statusMessages: string[];
  addStatusMessage: (msg: string) => void;

  loading: boolean;
  setLoading: (l: boolean) => void;

  getPersistedState: () => Record<string, string>;
  restorePersistedState: (data: Record<string, string>) => void;
}

const BASE_PROVIDERS: MetadataProvider[] = [
  { id: "tmdb", name: "TMDb", category: "Movies & TV", enabled: true },
  { id: "omdb", name: "OMDb", category: "Movies & TV", enabled: true },
  { id: "tvdb", name: "TVDB", category: "Movies & TV", enabled: true },
  { id: "trakt", name: "Trakt", category: "Movies & TV", enabled: true },
  { id: "imdb", name: "IMDb", category: "Movies & TV", enabled: true },
  { id: "rotten_tomatoes", name: "Rotten Tomatoes", category: "Movies & TV", enabled: true },
  { id: "cinemeta", name: "CINEMETA", category: "Movies & TV", enabled: true },
  { id: "tvmaze", name: "TVMaze", category: "Movies & TV", enabled: true },
  { id: "musicbrainz", name: "MusicBrainz", category: "Music", enabled: true },
  { id: "audiodb", name: "AudioDB", category: "Music", enabled: true },
  { id: "lastfm", name: "Last.fm", category: "Music", enabled: true },
  { id: "discogs", name: "Discogs", category: "Music", enabled: true },
  { id: "anidb", name: "AniDB", category: "Anime", enabled: true },
  { id: "anilist", name: "AniList", category: "Anime", enabled: true },
  { id: "myanimelist", name: "MyAnimeList", category: "Anime", enabled: true },
  { id: "kitsu", name: "Kitsu", category: "Anime", enabled: true },
  { id: "fanarttv", name: "Fanart.tv", category: "Artwork", enabled: true },
  { id: "tmdb_images", name: "TheMovieDB Images", category: "Artwork", enabled: true },
  { id: "opensubtitles", name: "OpenSubtitles", category: "Subtitles", enabled: true },
  { id: "subscene", name: "Subscene", category: "Subtitles", enabled: true },
  { id: "igdb", name: "IGDB", category: "Other", enabled: true },
  { id: "openlibrary", name: "OpenLibrary", category: "Other", enabled: true },
  { id: "goodreads", name: "GoodReads", category: "Other", enabled: true },
  { id: "epg_guide", name: "EPG Guide", category: "Other", enabled: true },
  { id: "plex_agents", name: "MS-A Agents", category: "Agents", enabled: true },
  { id: "emby_providers", name: "MS-B Providers", category: "Agents", enabled: true },
  { id: "jellyfin_providers", name: "MS-C Providers", category: "Agents", enabled: true },
];

function uniqueProviders(providers: MetadataProvider[]): MetadataProvider[] {
  const merged = new Map<string, MetadataProvider>();
  for (const provider of providers) {
    merged.set(provider.id, provider);
  }
  return Array.from(merged.values());
}

const DEFAULT_PROVIDERS: MetadataProvider[] = uniqueProviders([
  ...BASE_PROVIDERS,
  ...ADULT_METADATA_PROVIDERS.map(({ id, name, category, enabled }) => ({ id, name, category, enabled })),
]);

const DEFAULT_SCHEDULED_TASKS: ScheduledTaskConfig = {
  thumbnails: "on_scan",
  chapter_images: "on_scan",
  metadata_check: "daily",
  match_unmatch: "on_import",
};

const DEFAULT_FEATURE_SETTINGS: Record<string, { enabled: boolean; config: unknown }> = {
  smart_collections: { enabled: true, config: {} },
  poster_sync: { enabled: true, config: {} },
  unified_library: { enabled: true, config: {} },
  watchlist: { enabled: true, config: {} },
  skip_intro: { enabled: true, config: {} },
  skip_outro: { enabled: true, config: {} },
  auto_next: { enabled: true, config: {} },
  auto_subtitles: { enabled: true, config: {} },
  chapter_thumbs: { enabled: true, config: {} },
  hw_transcoding: { enabled: true, config: {} },
  motion_effects: { enabled: true, config: {} },
  splash_screen: { enabled: true, config: {} },
  particle_effects: { enabled: true, config: {} },
  ai_visualizer: { enabled: true, config: {} },
  glassmorphism: { enabled: true, config: {} },
  starfield_header: { enabled: true, config: {} },
  animated_sidebar: { enabled: true, config: {} },
  emby_sdk: { enabled: true, config: {} },
  vpn_integration: { enabled: true, config: {} },
  ai_diagnostics: { enabled: true, config: {} },
  duplicate_finder: { enabled: true, config: {} },
  iptv_support: { enabled: true, config: {} },
  plugin_system: { enabled: true, config: {} },
};

const DEFAULT_SETTINGS: Record<string, string> = {
  theme: "vidhub_flagship",
  splash_enabled: "true",
  sidebar_collapsed: "false",
  motion_enabled: "true",
  skip_intro: "true",
  skip_outro: "true",
  auto_next: "true",
  auto_subtitles: "true",
  chapter_thumbs_enabled: "true",
  prefer_embedded_titles: "true",
  smart_collections: "true",
  poster_sync: "true",
  unified_library: "true",
  watchlist_enabled: "true",
  hw_transcoding: "true",
  quality_control: "auto",
  remote_access_enabled: "true",
  remote_manually_specify_port: "false",
  remote_public_port: "32400",
  remote_secure_connections: "preferred",
  remote_preferred_relay: "false",
  remote_allow_fallback: "true",
  remote_upload_limit_mbps: "20",
  remote_allowed_networks: "",
  remote_enable_upnp: "true",
  remote_enable_natpmp: "true",
  default_player: "system",
  particle_effects: "true",
  ai_visualizer: "true",
  glassmorphism: "true",
  starfield_header: "true",
  window_opacity: "100",
  brand_logo_profile: "v125_ice_blue",
  startup_animation: "photorealistic_ice_blue_logo",
};

export const useAppStore = create<AppState>((set, get) => ({
  activeTab: "home",
  sidebarCollapsed: false,
  setActiveTab: (tab) => set({ activeTab: tab }),
  toggleSidebar: () => set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),

  currentTheme: "vidhub_flagship",
  setTheme: (theme) => set({ currentTheme: theme }),

  mediaItems: [],
  setMediaItems: (items) => set({ mediaItems: items }),
  selectedMedia: null,
  setSelectedMedia: (item) => set({ selectedMedia: item }),
  libraryView: "card",
  setLibraryView: (view) => set({ libraryView: view }),
  searchQuery: "",
  setSearchQuery: (q) => set({ searchQuery: q }),

  sources: [],
  setSources: (sources) => set({ sources }),

  scanning: false,
  scanProgress: { total: 0, current: 0 },
  setScanning: (s) => set({ scanning: s }),
  setScanProgress: (p) => set({ scanProgress: p }),

  serverRunning: false,
  serverType: "jellyfin",
  serverUrl: "http://localhost:8096",
  setServerStatus: (running, type_, url) => set({ serverRunning: running, serverType: type_, serverUrl: url }),

  downloading: false,
  setDownloading: (d) => set({ downloading: d }),

  vpnConnected: false,
  vpnLocation: "",
  setVpnStatus: (connected, location) => set({ vpnConnected: connected, vpnLocation: location }),

  aiProcessing: false,
  aiResult: null,
  setAiProcessing: (p) => set({ aiProcessing: p }),
  setAiResult: (r) => set({ aiResult: r }),

  settings: { ...DEFAULT_SETTINGS },
  setSettings: (s) => set({ settings: { ...DEFAULT_SETTINGS, ...s } }),
  setSetting: (key, value) => set((state) => ({ settings: { ...state.settings, [key]: value } })),

  featureSettings: { ...DEFAULT_FEATURE_SETTINGS },
  setFeatureSettings: (fs) => set({ featureSettings: fs }),
  toggleFeature: (key) => set((state) => {
    const current = state.featureSettings[key] || { enabled: false, config: {} };
    return {
      featureSettings: {
        ...state.featureSettings,
        [key]: { ...current, enabled: !current.enabled },
      },
    };
  }),

  metadataProviders: [...DEFAULT_PROVIDERS],
  setMetadataProviders: (p) => set({ metadataProviders: sanitizeMetadataProviders(p, DEFAULT_PROVIDERS) }),
  toggleMetadataProvider: (id) => set((state) => ({
    metadataProviders: state.metadataProviders.map((provider) =>
      provider.id === id ? { ...provider, enabled: !provider.enabled } : provider,
    ),
  })),
  enableAllProviders: (category) => set((state) => ({
    metadataProviders: state.metadataProviders.map((provider) =>
      !category || provider.category === category ? { ...provider, enabled: true } : provider,
    ),
  })),
  disableAllProviders: (category) => set((state) => ({
    metadataProviders: state.metadataProviders.map((provider) =>
      !category || provider.category === category ? { ...provider, enabled: false } : provider,
    ),
  })),

  scheduledTasks: { ...DEFAULT_SCHEDULED_TASKS },
  setScheduledTasks: (t) => set({ scheduledTasks: t }),
  setTaskFrequency: (task, freq) => set((state) => ({
    scheduledTasks: { ...state.scheduledTasks, [task]: freq },
  })),

  cloudServices: {
    onedrive: { id: "onedrive", status: "disconnected" },
    gdrive: { id: "gdrive", status: "disconnected" },
    dropbox: { id: "dropbox", status: "disconnected" },
  },
  setCloudService: (id, cloudState) => set((state) => ({
    cloudServices: {
      ...state.cloudServices,
      [id]: { ...(state.cloudServices[id] || { id, status: "disconnected" as CloudServiceStatus }), ...cloudState },
    },
  })),

  statusMessages: ["CinaVault Premium initialized", "All systems operational — Premium Edition"],
  addStatusMessage: (msg) => set((state) => ({
    statusMessages: [...state.statusMessages.slice(-19), msg],
  })),

  loading: false,
  setLoading: (l) => set({ loading: l }),

  getPersistedState: () => {
    const state = get();
    return {
      ...state.settings,
      _activeTab: state.activeTab,
      _sidebarCollapsed: String(state.sidebarCollapsed),
      _currentTheme: state.currentTheme,
      _libraryView: state.libraryView,
      _featureSettings: JSON.stringify(state.featureSettings),
      _metadataProviders: JSON.stringify(state.metadataProviders),
      _scheduledTasks: JSON.stringify(state.scheduledTasks),
      _cloudServices: JSON.stringify(state.cloudServices),
    };
  },

  restorePersistedState: (data) => {
    const settings: Record<string, string> = {};
    let activeTab: TabId = "home";
    let sidebarCollapsed = false;
    let currentTheme = "vidhub_flagship";
    let libraryView: "list" | "card" = "card";
    let featureSettings = { ...DEFAULT_FEATURE_SETTINGS };
    let metadataProviders = [...DEFAULT_PROVIDERS];
    let scheduledTasks = { ...DEFAULT_SCHEDULED_TASKS };
    let cloudServices = get().cloudServices;

    for (const [key, value] of Object.entries(data)) {
      if (key === "_activeTab") {
        activeTab = value as TabId;
      } else if (key === "_sidebarCollapsed") {
        sidebarCollapsed = value === "true";
      } else if (key === "_currentTheme") {
        currentTheme = value;
      } else if (key === "_libraryView") {
        libraryView = value as "list" | "card";
      } else if (key === "_featureSettings") {
        try { featureSettings = { ...DEFAULT_FEATURE_SETTINGS, ...JSON.parse(value) }; } catch {}
      } else if (key === "_metadataProviders") {
        try { metadataProviders = sanitizeMetadataProviders(JSON.parse(value), DEFAULT_PROVIDERS); } catch {}
      } else if (key === "_scheduledTasks") {
        try { scheduledTasks = { ...DEFAULT_SCHEDULED_TASKS, ...JSON.parse(value) }; } catch {}
      } else if (key === "_cloudServices") {
        try { cloudServices = { ...cloudServices, ...JSON.parse(value) }; } catch {}
      } else {
        settings[key] = value;
      }
    }

    set({
      settings: { ...DEFAULT_SETTINGS, ...settings },
      activeTab,
      sidebarCollapsed,
      currentTheme,
      libraryView,
      featureSettings,
      metadataProviders,
      scheduledTasks,
      cloudServices,
    });
  },
}));
