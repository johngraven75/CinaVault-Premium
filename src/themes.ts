// CinaVault Premium — Theme System (6 presets, 16 color tokens each)

export interface ThemePreset {
  id: string;
  name: string;
  colors: Record<string, string>;
}

export const THEME_PRESETS: ThemePreset[] = [
  {
    id: "vidhub_flagship",
    name: "CinaVault Quantum",
    colors: {
      bg: "#050711", panel: "rgba(8, 18, 31, 0.9)", "panel-2": "rgba(13, 27, 43, 0.82)",
      "panel-3": "rgba(19, 36, 56, 0.72)", text: "#f4fbff", subtext: "#a7c8db",
      accent: "#00eaff", "accent-2": "#ff7a3d", "accent-3": "#92ff68",
      gold: "#ffe66d", "row-a": "rgba(6, 18, 32, 0.72)", "row-b": "rgba(12, 30, 48, 0.68)",
      danger: "#ff355f", "neon-1": "#f7fbff", "neon-2": "#00eaff", "neon-3": "#ff4db8",
    },
  },
  {
    id: "emby_sdk",
    name: "MS-B Classic",
    colors: {
      bg: "#f0f0f5", panel: "#ffffff", "panel-2": "#f8f8fc", "panel-3": "#eeeef4",
      text: "#1a1a2e", subtext: "#6b6b8a", accent: "#52b54b", "accent-2": "#4ba34b",
      "accent-3": "#3d8b3d", gold: "#d4a017", "row-a": "#ffffff", "row-b": "#f5f5fa",
      danger: "#e74c3c", "neon-1": "#52b54b", "neon-2": "#4ba34b", "neon-3": "#3d8b3d",
    },
  },
  {
    id: "jellyfin_glass",
    name: "MS-C Aurora",
    colors: {
      bg: "#0a0a1a", panel: "rgba(20,15,40,0.85)", "panel-2": "rgba(30,20,60,0.75)",
      "panel-3": "rgba(40,25,80,0.65)", text: "#e8e6f0", subtext: "#9b95b0",
      accent: "#a78bfa", "accent-2": "#7c5cbf", "accent-3": "#6d4aad",
      gold: "#fbbf24", "row-a": "rgba(25,18,50,0.6)", "row-b": "rgba(35,25,65,0.6)",
      danger: "#ef4444", "neon-1": "#c084fc", "neon-2": "#a78bfa", "neon-3": "#8b5cf6",
    },
  },
  {
    id: "windows_11_glass",
    name: "Windows 11 Glass",
    colors: {
      bg: "#0a1628", panel: "rgba(15,25,50,0.85)", "panel-2": "rgba(20,35,65,0.75)",
      "panel-3": "rgba(25,40,75,0.65)", text: "#e0e8f5", subtext: "#8899bb",
      accent: "#60a5fa", "accent-2": "#3b82f6", "accent-3": "#2563eb",
      gold: "#f59e0b", "row-a": "rgba(15,25,55,0.6)", "row-b": "rgba(20,35,70,0.6)",
      danger: "#ef4444", "neon-1": "#93c5fd", "neon-2": "#60a5fa", "neon-3": "#3b82f6",
    },
  },
  {
    id: "neon_night",
    name: "Neon Night",
    colors: {
      bg: "#0a0f0a", panel: "rgba(10,25,15,0.85)", "panel-2": "rgba(15,35,20,0.75)",
      "panel-3": "rgba(20,45,25,0.65)", text: "#d0f0d8", subtext: "#6aaa78",
      accent: "#22c55e", "accent-2": "#16a34a", "accent-3": "#15803d",
      gold: "#eab308", "row-a": "rgba(10,30,15,0.6)", "row-b": "rgba(15,40,20,0.6)",
      danger: "#ef4444", "neon-1": "#4ade80", "neon-2": "#22c55e", "neon-3": "#16a34a",
    },
  },
  {
    id: "ocean_drive",
    name: "Ocean Drive",
    colors: {
      bg: "#0a1a1f", panel: "rgba(10,30,40,0.85)", "panel-2": "rgba(15,40,55,0.75)",
      "panel-3": "rgba(20,50,65,0.65)", text: "#d0f0f8", subtext: "#6aaabb",
      accent: "#06b6d4", "accent-2": "#0891b2", "accent-3": "#0e7490",
      gold: "#f59e0b", "row-a": "rgba(10,35,45,0.6)", "row-b": "rgba(15,45,60,0.6)",
      danger: "#ef4444", "neon-1": "#67e8f9", "neon-2": "#22d3ee", "neon-3": "#06b6d4",
    },
  },
  {
    id: "sunset_pulse",
    name: "Sunset Pulse",
    colors: {
      bg: "#1a0a0a", panel: "rgba(40,15,15,0.85)", "panel-2": "rgba(55,20,20,0.75)",
      "panel-3": "rgba(70,25,25,0.65)", text: "#f0e0d8", subtext: "#bb8a7a",
      accent: "#f97316", "accent-2": "#ea580c", "accent-3": "#c2410c",
      gold: "#fbbf24", "row-a": "rgba(45,15,15,0.6)", "row-b": "rgba(60,20,20,0.6)",
      danger: "#ef4444", "neon-1": "#fdba74", "neon-2": "#fb923c", "neon-3": "#f97316",
    },
  },
  {
    id: "mediafire_fusion",
    name: "MediaFire Fusion",
    colors: {
      bg: "#081326", panel: "rgba(16, 24, 43, 0.84)", "panel-2": "rgba(22, 32, 54, 0.76)",
      "panel-3": "rgba(29, 40, 66, 0.66)", text: "#eef4ff", subtext: "#b9cadf",
      accent: "#ff4b2d", "accent-2": "#ff7a1f", "accent-3": "#1d8fe3",
      gold: "#ffb83d", "row-a": "rgba(10, 23, 44, 0.6)", "row-b": "rgba(16, 30, 58, 0.6)",
      danger: "#ff2f43", "neon-1": "#59b6ff", "neon-2": "#ff8330", "neon-3": "#ff4637",
    },
  },
];

export function applyTheme(themeId: string): void {
  const theme = THEME_PRESETS.find((t) => t.id === themeId) || THEME_PRESETS[1];
  const root = document.documentElement;
  Object.entries(theme.colors).forEach(([key, value]) => {
    root.style.setProperty(`--cv-${key}`, value);
  });
  root.setAttribute("data-theme", theme.id);
}

export function getThemeById(id: string): ThemePreset {
  return THEME_PRESETS.find((t) => t.id === id) || THEME_PRESETS[1];
}
