export const REIMAGINED_FEATURES = {
  localLibraries: true,
  movies: true,
  television: true,
  music: true,
  liveTv: true,
  metadataEnrichment: true,
  subtitles: true,
  playlists: true,
  watchHistory: true,
  multiUserProfiles: true,
  parentalControls: true,
  chromecast: true,
  airPlay: true,
  smartView: true,
  networkDeviceDiscovery: true,
  remoteStreaming: true,
  cloudLibraries: true,
  adaptiveStreaming: true,
  serverlessControlPlane: true,
  activityDashboard: true,
} as const;

export type ReImaginedFeature = keyof typeof REIMAGINED_FEATURES;
