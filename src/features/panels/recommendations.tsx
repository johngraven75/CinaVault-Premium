// Settings for AI Recommendations: how many picks the shelf shows.
import type { FeaturePanel } from "./types";
import { RECOMMENDATIONS_DEFAULTS, type RecommendationsConfig } from "../../services/discoveryLogic";

const RecommendationsPanel: FeaturePanel<RecommendationsConfig> = ({ config, setConfig }) => (
  <div className="space-y-2 text-xs">
    <label className="flex items-center justify-between gap-3">
      <span>Titles on the shelf</span>
      <input
        className="cv-input w-24 text-xs"
        type="number"
        min={4}
        max={60}
        value={config.limit ?? RECOMMENDATIONS_DEFAULTS.limit}
        onChange={(event) => void setConfig({ limit: Number(event.target.value) })}
      />
    </label>
    <p className="text-cv-subtext">
      Ranked on this PC from the active profile's watched, in-progress, favorite and watchlist titles by shared genres,
      release year and rating. Nothing leaves the machine.
    </p>
  </div>
);

export default RecommendationsPanel;
