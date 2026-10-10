// Auto Collections: grouping limits, current collections and "Rebuild now".
import { useCallback, useEffect, useState } from "react";

import type { FeaturePanel } from "./types";
import { boundedInt } from "../library/libraryAutomation.ts";
import { groupCollections, listCollections, rebuildCollections, type CollectionSummary } from "../../services/collections";

interface CollectionConfig {
  topGenres?: number;
  minItems?: number;
}

const CollectionAutoPanel: FeaturePanel<CollectionConfig> = ({ enabled, config, setConfig }) => {
  const [collections, setCollections] = useState<CollectionSummary[]>([]);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");

  const refresh = useCallback(() => {
    listCollections().then(setCollections).catch((error) => setMessage(String(error)));
  }, []);
  useEffect(refresh, [refresh, enabled]);

  const rebuild = async () => {
    setBusy(true);
    try {
      const report = await rebuildCollections();
      setMessage(
        `${report.franchises} franchise, ${report.series} series and ${report.genres} genre collection(s)` +
          (report.franchiseLookups ? `; ${report.franchiseLookups} TMDB lookup(s)` : "") +
          (report.errors.length ? `; ${report.errors.length} error(s)` : ""),
      );
      refresh();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-2 text-xs">
      <div className="flex flex-wrap items-center gap-3">
        <label className="flex items-center gap-1">
          <span>Genre collections</span>
          <input
            type="number"
            min={1}
            max={30}
            className="cv-input w-14 text-xs"
            defaultValue={boundedInt(config.topGenres, 8, 1, 30)}
            onBlur={(event) => void setConfig({ topGenres: boundedInt(event.target.value, 8, 1, 30) })}
          />
        </label>
        <label className="flex items-center gap-1">
          <span>Smallest collection</span>
          <input
            type="number"
            min={2}
            max={50}
            className="cv-input w-14 text-xs"
            defaultValue={boundedInt(config.minItems, 2, 2, 50)}
            onBlur={(event) => void setConfig({ minItems: boundedInt(event.target.value, 2, 2, 50) })}
          />
          <span>titles</span>
        </label>
        <button
          type="button"
          className="cv-btn cv-btn-secondary text-[10px] px-2 py-1"
          disabled={!enabled || busy}
          onClick={() => void rebuild()}
        >
          {busy ? "Rebuilding..." : "Rebuild now"}
        </button>
      </div>
      <div className="text-cv-subtext">
        Series come from episode names and tvshow.nfo, franchises from NFO sets or TMDB (with a TMDB key), genres from
        your movies. Collections are rebuilt after every scan.
      </div>
      {groupCollections(collections).map((group) => (
        <div key={group.kind}>
          <span className="text-cv-subtext">{group.label}: </span>
          {group.collections
            .slice(0, 8)
            .map((collection) => `${collection.name} (${collection.itemCount})`)
            .join(", ")}
          {group.collections.length > 8 ? `, +${group.collections.length - 8} more` : ""}
        </div>
      ))}
      {enabled && collections.length === 0 && <div className="text-cv-subtext">No collections yet.</div>}
      {message && <div className="text-cv-subtext" role="status">{message}</div>}
    </div>
  );
};

export default CollectionAutoPanel;
