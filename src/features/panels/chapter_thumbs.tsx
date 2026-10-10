// Chapter Thumbnails: spacing of the frames generated after a scan.
import { useState } from "react";

import type { FeaturePanel } from "./types";
import { chapterIntervalMinutes } from "../library/libraryAutomation.ts";

const ChapterThumbsPanel: FeaturePanel<{ intervalMinutes?: number }> = ({ config, setConfig }) => {
  const [value, setValue] = useState(String(chapterIntervalMinutes(config.intervalMinutes)));
  const save = async () => {
    const minutes = chapterIntervalMinutes(value);
    setValue(String(minutes));
    await setConfig({ intervalMinutes: minutes });
  };
  return (
    <div className="space-y-2 text-xs">
      <label className="flex items-center gap-2">
        <span>One frame every</span>
        <input
          type="number"
          min={0.5}
          max={60}
          step={0.5}
          className="cv-input w-20 text-xs"
          value={value}
          onChange={(event) => setValue(event.target.value)}
          onBlur={() => void save()}
        />
        <span>minutes</span>
      </label>
      <div className="text-cv-subtext">
        After a scan, new videos get frames in a "&lt;name&gt;_chapters" folder next to the file (needs FFmpeg).
        Videos that already have one are skipped. Off: frames are only made when you ask for them.
      </div>
    </div>
  );
};

export default ChapterThumbsPanel;
