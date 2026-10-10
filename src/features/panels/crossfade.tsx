import type { FeaturePanel } from "./types";
import { NumberField } from "../../components/player/PanelFields";

const CrossfadePanel: FeaturePanel = ({ config, setConfig }) => (
  <NumberField
    label="Fade length"
    value={Number(config.seconds ?? 1.5)}
    min={0.2}
    max={10}
    step={0.1}
    unit="s"
    onCommit={(seconds) => void setConfig({ seconds })}
  />
);

export default CrossfadePanel;
