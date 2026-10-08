import type { FeaturePanel } from "./types";
import { NumberField, ToggleField } from "../../components/player/PanelFields";

const SkipCreditsPanel: FeaturePanel = ({ config, setConfig }) => (
  <div className="space-y-2">
    <NumberField
      label="Credits length when the file has no credits chapter (0 = chapters only)"
      value={Number(config.creditsSeconds ?? 120)}
      min={0}
      max={1200}
      unit="s"
      onCommit={(creditsSeconds) => void setConfig({ creditsSeconds })}
    />
    <ToggleField
      label="Skip automatically (plays the next title, or stops)"
      checked={Boolean(config.autoSkip)}
      onChange={(autoSkip) => void setConfig({ autoSkip })}
    />
  </div>
);

export default SkipCreditsPanel;
