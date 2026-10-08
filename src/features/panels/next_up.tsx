import type { FeaturePanel } from "./types";
import { NumberField } from "../../components/player/PanelFields";

const NextUpPanel: FeaturePanel = ({ config, setConfig }) => (
  <div className="space-y-2">
    <NumberField
      label="Countdown before the next title plays"
      value={Number(config.countdownSeconds ?? 10)}
      min={1}
      max={120}
      unit="s"
      onCommit={(countdownSeconds) => void setConfig({ countdownSeconds })}
    />
    <NumberField
      label="Show this long before the end when there is no credits marker"
      value={Number(config.secondsBeforeEnd ?? 30)}
      min={5}
      max={600}
      unit="s"
      onCommit={(secondsBeforeEnd) => void setConfig({ secondsBeforeEnd })}
    />
  </div>
);

export default NextUpPanel;
