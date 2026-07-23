import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, access } from 'node:fs/promises';
import { resolve } from 'node:path';

const repoRoot = resolve(import.meta.dirname, '../../..');

const requiredLegacyCapabilities = [
  ['Tauri shell', 'src/App.tsx', ['Build 140 Futuristic Application Shell']],
  ['SQLite database', 'src-tauri/Cargo.toml', ['rusqlite']],
  ['Media scanner', 'src-tauri/src/scanner.rs', ['scan_sources', 'scan_library']],
  ['Media playback', 'src-tauri/src/player.rs', ['play_media']],
  ['Download manager', 'src-tauri/src/downloads.rs', ['start_download']],
  ['IPTV and Live TV', 'src-tauri/src/iptv.rs', ['get_live_channels', 'get_iptv_channels']],
  ['Jellyfin fallback', 'src-tauri/src/jellyfin.rs', ['jellyfin']],
  ['VPN integration', 'src-tauri/src/vpn.rs', ['vpn']],
  ['Chapter detection', 'src-tauri/src/chapters.rs', ['detect_chapters']],
  ['Duplicate detection', 'src-tauri/src/duplicates.rs', ['find_duplicates']],
  ['Task progress', 'src-tauri/src/task_progress.rs', ['TaskProgress']],
  ['Google Cast', 'src/services/googleCast.ts', ['CastSession']],
  ['Metadata extensions', 'src-tauri/src/metadata_ext.rs', ['metadata_ext']],
  ['PGMA bridge', 'src-tauri/src/pgma_bridge.rs', ['pgma_bridge']],
  ['Adult provider', 'src-tauri/src/adult_site_provider.rs', ['adult_site_provider']],
  ['AI media agent', 'src/services/aiMediaAgent.ts', ['aiMediaAgent']],
  ['CinaVault server', 'src/services/serverProvider.ts', ['cinavaultServer']],
  ['AI fallback', 'src/services/aiProviderFallback.ts', ['aiProviderFallback']],
  ['Permanent plugins', 'src/plugins/permanentMediaPlugins.ts', ['permanentMediaPlugins']],
  ['Poster write-back', 'src-tauri/src/enrichment.rs', ['download_poster_to_sidecar']],
  ['NFO write-back', 'src-tauri/src/enrichment.rs', ['write_nfo_sidecar']],
  ['Plugin repositories', 'src-tauri/src/plugins.rs', ['get_plugin_repos']],
  ['Plugin install', 'src-tauri/src/plugins.rs', ['install_plugin']],
  ['Synology NAS', 'src-tauri/src/nas_devices.rs', ['synology_connect']],
  ['WD My Cloud', 'src-tauri/src/nas_devices.rs', ['wd_mycloud_connect']],
  ['Media tool bootstrap', 'src-tauri/src/media_tools.rs', ['ensure_media_tools']],
  ['Real source discovery', 'src-tauri/src/scanner.rs', ['discover_and_add_sources']],
  ['Atomic poster sidecars', 'src-tauri/src/enrichment.rs', ['write_poster_sidecar_bytes']],
];

const requiredTabs = [
  'HomeTab', 'MediaSourcesTab', 'DownloadsTab', 'LiveTVTab', 'ServerTab',
  'SecurityTab', 'RemoteAccessTab', 'AdvancedTab', 'CloudNASTab',
  'PluginsTab', 'AIDiagnosticsTab', 'SettingsTab'
];

test('legacy capability sources remain intact during migration', async () => {
  for (const [name, path, acceptedTokens] of requiredLegacyCapabilities) {
    const absolute = resolve(repoRoot, path);
    await access(absolute);
    const content = await readFile(absolute, 'utf8');
    assert.ok(
      acceptedTokens.some((token) => content.includes(token)),
      `${name} missing accepted implementation token (${acceptedTokens.join(' or ')}) in ${path}`,
    );
  }
});

test('ReImagined feature contract exposes every user-facing area', async () => {
  const app = await readFile(resolve(repoRoot, 'apps/cinavault-reimagined/src/core/features.ts'), 'utf8');
  const parity = await readFile(resolve(repoRoot, 'apps/cinavault-reimagined/src/core/parity.ts'), 'utf8');
  for (const tab of requiredTabs) {
    assert.ok(parity.includes(tab), `ReImagined parity contract does not include ${tab}`);
  }
  assert.ok(app.includes('cloud'), 'Cloud capability missing from ReImagined feature registry');
  assert.ok(app.includes('cast'), 'Casting capability missing from ReImagined feature registry');
});
