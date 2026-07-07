export const permanentMediaPlugins: any[] = [];

export function arePermanentMediaPluginsReady(): boolean {
  return true;
}

export function getStartupMediaPlugins() {
  return permanentMediaPlugins;
}
