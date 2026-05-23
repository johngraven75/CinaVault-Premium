// CinaVault Premium — Global State Store (Zustand)
import { create } from "zustand";

export type TabId =
  | "home" | "sources" | "downloads" | "livetv" | "server"
  | "security" | "advanced" | "cloud" | "plugins" | "ai" | "mediaactions" | "settings";

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
   // For alphabetical view headers
   isHeader?: boolean;
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

export interface AppState {
  // Navigation
  activeTab: TabId;
  sidebarCollapsed: boolean;
  setActiveTab: (tab: TabId) => void;
  toggleSidebar: () => void;

  // Theme
  currentTheme: string;
  setTheme: (theme: string) => void;

  // Library
  mediaItems: MediaItem[];
  setMediaItems: (items: MediaItem[]) => void;
  selectedMedia: MediaItem | null;
  setSelectedMedia: (item: MediaItem | null) => void;
  libraryView: "list" | "card";
  setLibraryView: (view: "list" | "card") => void;
  searchQuery: string;
  setSearchQuery: (q: string) => void;

  // Sources
  sources: MediaSource[];
  setSources: (sources: MediaSource[]) => void;

  // Scanning
  scanning: boolean;
  scanProgress: { total: number; current: number };
  setScanning: (s: boolean) => void;
  setScanProgress: (p: { total: number; current: number }) => void;

  // Server
  serverRunning: boolean;
  serverType: string;
  serverUrl: string;
  setServerStatus: (running: boolean, type_: string, url: string) => void;

  // Downloads
  downloading: boolean;
  setDownloading: (d: boolean) => void;

  // VPN
  vpnConnected: boolean;
  vpnLocation: string;
  setVpnStatus: (connected: boolean, location: string) => void;

  // AI
  aiProcessing: boolean;
  aiResult: any;
  setAiProcessing: (p: boolean) => void;
  setAiResult: (r: any) => void;

  // Settings
  settings: Record<string, string>;
  setSettings: (s: Record<string, string>) => void;
  setSetting: (key: string, value: string) => void;

  // Feature Settings (Advanced tab)
  featureSettings: Record<string, { enabled: boolean; config: any }>;
  setFeatureSettings: (fs: Record<string, { enabled: boolean; config: any }>) => void;
  toggleFeature: (key: string) => void;

  // Status ticker
  statusMessages: string[];
  addStatusMessage: (msg: string) => void;

  // Loading
  loading: boolean;
  setLoading: (l: boolean) => void;
}

export const useAppStore = create<AppState>((set, get) => ({
  // Navigation
  activeTab: "home",
  sidebarCollapsed: false,
  setActiveTab: (tab) => set({ activeTab: tab }),
  toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),

  // Theme
  currentTheme: "prism_fusion",
  setTheme: (theme) => set({ currentTheme: theme }),

  // Library
  mediaItems: [],
  setMediaItems: (items) => set({ mediaItems: items }),
  selectedMedia: null,
  setSelectedMedia: (item) => set({ selectedMedia: item }),
  libraryView: "card",
  setLibraryView: (view) => set({ libraryView: view }),
  searchQuery: "",
  setSearchQuery: (q) => set({ searchQuery: q }),

  // Sources
  sources: [],
  setSources: (sources) => set({ sources }),

  // Scanning
  scanning: false,
  scanProgress: { total: 0, current: 0 },
  setScanning: (s) => set({ scanning: s }),
  setScanProgress: (p) => set({ scanProgress: p }),

  // Server
  serverRunning: false,
  serverType: "jellyfin",
  serverUrl: "http://localhost:8096",
  setServerStatus: (running, type_, url) => set({ serverRunning: running, serverType: type_, serverUrl: url }),

  // Downloads
  downloading: false,
  setDownloading: (d) => set({ downloading: d }),

  // VPN
  vpnConnected: false,
  vpnLocation: "",
  setVpnStatus: (connected, location) => set({ vpnConnected: connected, vpnLocation: location }),

  // AI
  aiProcessing: false,
  aiResult: null,
  setAiProcessing: (p) => set({ aiProcessing: p }),
  setAiResult: (r) => set({ aiResult: r }),

  // Settings
  settings: {},
  setSettings: (s) => set({ settings: s }),
  setSetting: (key, value) => set((s) => ({ settings: { ...s.settings, [key]: value } })),

  // Feature Settings
  featureSettings: {},
  setFeatureSettings: (fs) => set({ featureSettings: fs }),
  toggleFeature: (key) => set((s) => {
    const current = s.featureSettings[key] || { enabled: false, config: {} };
    return {
      featureSettings: {
        ...s.featureSettings,
        [key]: { ...current, enabled: !current.enabled },
      },
    };
  }),

  // Status
  statusMessages: ["CinaVault Premium initialized", "All systems operational"],
  addStatusMessage: (msg) => set((s) => ({
    statusMessages: [...s.statusMessages.slice(-19), msg],
  })),

  // Loading
  loading: false,
  setLoading: (l) => set({ loading: l }),
}));
