// Settings for Genre Radio Stations: how many titles a station queues.
import type { FeaturePanel } from "./types";
import { GENRE_RADIO_DEFAULTS, type GenreRadioConfig } from "../../services/discoveryLogic";

const GenreRadioPanel: FeaturePanel<GenreRadioConfig> = ({ config, setConfig }) => (
  <div className="space-y-2 text-xs">
    <label className="flex items-center justify-between gap-3">
      <span>Titles per station</span>
      <input
        className="cv-input w-24 text-xs"
        type="number"
        min={5}
        max={1000}
        value={config.queueSize ?? GENRE_RADIO_DEFAULTS.queueSize}
        onChange={(event) => void setConfig({ queueSize: Number(event.target.value) })}
      />
    </label>
    <p className="text-cv-subtext">The station shuffles every playable title of the chosen genre, up to this many.</p>
  </div>
);

export default GenreRadioPanel;
