// Contract for a switch's settings panel. A file src/features/panels/<key>.tsx
// whose default export is a FeaturePanel appears under that switch in
// Advanced > Feature Matrix; AdvancedTab finds panels by file name.
import type { FC } from "react";

export interface FeaturePanelProps<T extends object = Record<string, any>> {
  enabled: boolean;
  config: T;
  /** Merges into the saved config and persists it. */
  setConfig: (next: Partial<T>) => Promise<void>;
}

export type FeaturePanel<T extends object = Record<string, any>> = FC<FeaturePanelProps<T>>;
