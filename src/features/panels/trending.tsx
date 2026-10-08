// Settings for Trending Content: where the shelf comes from and how long it is.
import type { FeaturePanel } from "./types";
import { TRENDING_DEFAULTS, type TrendingConfig } from "../../services/discoveryLogic";

const TrendingPanel: FeaturePanel<TrendingConfig> = ({ config, setConfig }) => {
  const source = config.source ?? TRENDING_DEFAULTS.source;
  const limit = config.limit ?? TRENDING_DEFAULTS.limit;
  return (
    <div className="space-y-2 text-xs">
      <label className="flex items-center justify-between gap-3">
        <span>Source</span>
        <select
          className="cv-input w-56 text-xs"
          value={source}
          onChange={(event) => void setConfig({ source: event.target.value as TrendingConfig["source"] })}
        >
          <option value="auto">TMDB weekly trending (needs a TMDB key)</option>
          <option value="local">Most played on this PC</option>
        </select>
      </label>
      <label className="flex items-center justify-between gap-3">
        <span>Titles on the shelf</span>
        <input
          className="cv-input w-24 text-xs"
          type="number"
          min={4}
          max={60}
          value={limit}
          onChange={(event) => void setConfig({ limit: Number(event.target.value) })}
        />
      </label>
      <p className="text-cv-subtext">
        TMDB results are cached for 6 hours. Without a key, or offline with nothing cached, the shelf ranks titles played on this PC in the last 30 days.
      </p>
    </div>
  );
};

export default TrendingPanel;
