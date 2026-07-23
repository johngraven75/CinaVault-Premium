export type ParityStatus = 'legacy-backed' | 'native-reimagined' | 'migration-required';

export interface ParityCapability {
  id: string;
  label: string;
  surface: string;
  status: ParityStatus;
}

export const parityCapabilities: ParityCapability[] = [
  { id: 'home', label: 'Home and media shelves', surface: 'HomeTab', status: 'legacy-backed' },
  { id: 'sources', label: 'Media source discovery, scan, repair and verification', surface: 'MediaSourcesTab', status: 'legacy-backed' },
  { id: 'downloads', label: 'Downloads and acquisition queue', surface: 'DownloadsTab', status: 'legacy-backed' },
  { id: 'livetv', label: 'IPTV, channels, guide and live playback', surface: 'LiveTVTab', status: 'legacy-backed' },
  { id: 'server', label: 'CinaVault server and Jellyfin fallback', surface: 'ServerTab', status: 'legacy-backed' },
  { id: 'security', label: 'Security, credentials and access controls', surface: 'SecurityTab', status: 'legacy-backed' },
  { id: 'remote', label: 'Remote access, relay and VPN', surface: 'RemoteAccessTab', status: 'legacy-backed' },
  { id: 'advanced', label: 'Advanced repair, chapters, duplicates and diagnostics', surface: 'AdvancedTab', status: 'legacy-backed' },
  { id: 'cloud-nas', label: 'Cloud, Synology and WD My Cloud libraries', surface: 'CloudNASTab', status: 'legacy-backed' },
  { id: 'plugins', label: 'Plugin catalog, install, uninstall and execution', surface: 'PluginsTab', status: 'legacy-backed' },
  { id: 'ai', label: 'AI diagnostics, routing and provider fallback', surface: 'AIDiagnosticsTab', status: 'legacy-backed' },
  { id: 'settings', label: 'Themes, preferences and persistent settings', surface: 'SettingsTab', status: 'legacy-backed' },
  { id: 'playback', label: 'Local and remote media playback', surface: 'Rust player commands', status: 'legacy-backed' },
  { id: 'casting', label: 'Google Cast, AirPlay and Smart View device playback', surface: 'Casting service layer', status: 'migration-required' },
  { id: 'metadata', label: 'Metadata routing, enrichment and sidecars', surface: 'Metadata and enrichment engines', status: 'legacy-backed' },
  { id: 'tools', label: 'FFmpeg, YT-DLP and MediaInfo bootstrap', surface: 'Permanent media tools', status: 'legacy-backed' },
  { id: 'database', label: 'SQLite media and settings database', surface: 'Rust database layer', status: 'legacy-backed' },
  { id: 'serverless', label: 'Cloud control plane and adaptive delivery', surface: 'ReImagined cloud architecture', status: 'migration-required' },
];

export const parityReleaseGate = {
  requireAllLegacyCapabilities: true,
  forbidSilentRemoval: true,
  requireWindowsInstaller: true,
  requireCarryForwardTests: true,
} as const;
