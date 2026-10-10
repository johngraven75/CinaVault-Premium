// Control bar for the built-in player: seek bar, transport, volume, fullscreen.
import type { FC } from "react";
import {
  Maximize2,
  Minimize2,
  Pause,
  Play,
  RotateCcw,
  RotateCw,
  SkipBack,
  SkipForward,
  Volume2,
  VolumeX,
} from "lucide-react";

import { formatClock } from "../../services/playerLogic";

export interface PlayerControlsProps {
  playing: boolean;
  time: number;
  duration: number;
  volume: number;
  muted: boolean;
  fullscreen: boolean;
  hasPrevious: boolean;
  hasNext: boolean;
  onTogglePlay: () => void;
  onSeek: (seconds: number) => void;
  onJump: (delta: number) => void;
  onVolume: (volume: number) => void;
  onToggleMute: () => void;
  onToggleFullscreen: () => void;
  onPrevious: () => void;
  onNext: () => void;
}

const iconButton =
  "flex h-9 w-9 items-center justify-center rounded-lg text-white/85 transition hover:bg-white/10 hover:text-white focus-visible:outline focus-visible:outline-2 focus-visible:outline-cyan-300 disabled:opacity-30";

const PlayerControls: FC<PlayerControlsProps> = (props) => {
  const { playing, time, duration, volume, muted, fullscreen } = props;
  const max = duration > 0 ? duration : Math.max(time, 1);
  return (
    <div className="flex flex-col gap-1.5">
      <input
        type="range"
        min={0}
        max={max}
        step={1}
        value={Math.min(time, max)}
        onChange={(event) => props.onSeek(Number(event.target.value))}
        aria-label="Seek"
        aria-valuetext={`${formatClock(time)} of ${formatClock(duration)}`}
        className="w-full accent-cyan-300"
      />
      <div className="flex items-center gap-1 text-xs text-white/80">
        <button type="button" className={iconButton} onClick={props.onPrevious} disabled={!props.hasPrevious} aria-label="Previous title">
          <SkipBack size={16} />
        </button>
        <button type="button" className={iconButton} onClick={() => props.onJump(-10)} aria-label="Back 10 seconds">
          <RotateCcw size={16} />
        </button>
        <button type="button" className={iconButton} onClick={props.onTogglePlay} aria-label={playing ? "Pause" : "Play"}>
          {playing ? <Pause size={18} /> : <Play size={18} />}
        </button>
        <button type="button" className={iconButton} onClick={() => props.onJump(10)} aria-label="Forward 10 seconds">
          <RotateCw size={16} />
        </button>
        <button type="button" className={iconButton} onClick={props.onNext} disabled={!props.hasNext} aria-label="Next title">
          <SkipForward size={16} />
        </button>
        <span className="ml-2 tabular-nums" aria-live="off">
          {formatClock(time)} / {duration > 0 ? formatClock(duration) : "--:--"}
        </span>
        <div className="ml-auto flex items-center gap-1">
          <button type="button" className={iconButton} onClick={props.onToggleMute} aria-label={muted ? "Unmute" : "Mute"}>
            {muted || volume === 0 ? <VolumeX size={16} /> : <Volume2 size={16} />}
          </button>
          <input
            type="range"
            min={0}
            max={1}
            step={0.05}
            value={muted ? 0 : volume}
            onChange={(event) => props.onVolume(Number(event.target.value))}
            aria-label="Volume"
            className="w-24 accent-cyan-300"
          />
          <button
            type="button"
            className={iconButton}
            onClick={props.onToggleFullscreen}
            aria-label={fullscreen ? "Exit full screen" : "Full screen"}
          >
            {fullscreen ? <Minimize2 size={16} /> : <Maximize2 size={16} />}
          </button>
        </div>
      </div>
    </div>
  );
};

export default PlayerControls;
