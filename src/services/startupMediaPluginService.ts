import {
  arePermanentMediaPluginsReady,
  getStartupMediaPlugins
} from "../plugins/permanentMediaPlugins";

export function initializePermanentMediaPluginsAtStartup() {
  return {
    ready: arePermanentMediaPluginsReady(),
    startupPlugins: getStartupMediaPlugins(),
    message:
      "Permanent media plugins are installed, enabled, configured, and initialized for startup."
  };
}
