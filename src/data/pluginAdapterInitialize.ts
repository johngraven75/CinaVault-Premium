import { pluginEngine } from "./pluginAdapter";

declare module "./pluginAdapter" {
  interface PluginAdapterEngine {
    initialize(): Promise<void>;
  }
}

if (typeof pluginEngine.initialize !== "function") {
  pluginEngine.initialize = async () => {
    await pluginEngine.loadFromBackend();
  };
}

export {};
