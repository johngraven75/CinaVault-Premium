// GPU acceleration takes effect when the app window is next created.
import type { FeaturePanel } from "./types";

const GpuAccelPanel: FeaturePanel = ({ enabled }) => (
  <div className="space-y-1.5 text-xs">
    <p>
      {enabled
        ? "Hardware rendering and platform video decoding (including HEVC where the system has a decoder) are used."
        : "The window renders in software. Use this if video or effects flicker or show black frames."}
    </p>
    <p className="text-[11px] text-amber-200">Takes effect after restarting CinaVault.</p>
  </div>
);

export default GpuAccelPanel;
