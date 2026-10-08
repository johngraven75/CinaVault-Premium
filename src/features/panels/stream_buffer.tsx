import type { FeaturePanel } from "./types";
import { NumberField } from "../../components/player/PanelFields";

const StreamBufferPanel: FeaturePanel = ({ config, setConfig }) => (
  <div className="space-y-2">
    <NumberField
      label="Buffer this much before resuming after a stall"
      value={Number(config.bufferAheadSeconds ?? 30)}
      min={5}
      max={300}
      unit="s"
      onCommit={(bufferAheadSeconds) => void setConfig({ bufferAheadSeconds })}
    />
    <NumberField
      label="Server read size for transcoded streams"
      value={Number(config.chunkKb ?? 256)}
      min={16}
      max={4096}
      unit="KB"
      onCommit={(chunkKb) => void setConfig({ chunkKb })}
    />
  </div>
);

export default StreamBufferPanel;
