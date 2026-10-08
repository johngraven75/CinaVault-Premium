import type { FeaturePanel } from "./types";
import { NumberField, ToggleField } from "../../components/player/PanelFields";

const SkipIntroPanel: FeaturePanel = ({ config, setConfig }) => (
  <div className="space-y-2">
    <NumberField
      label="Intro length when the file has no intro chapter (0 = chapters only)"
      value={Number(config.introSeconds ?? 90)}
      min={0}
      max={600}
      unit="s"
      onCommit={(introSeconds) => void setConfig({ introSeconds })}
    />
    <ToggleField
      label="Skip automatically"
      checked={Boolean(config.autoSkip)}
      onChange={(autoSkip) => void setConfig({ autoSkip })}
    />
  </div>
);

export default SkipIntroPanel;
