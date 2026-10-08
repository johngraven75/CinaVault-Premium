// "More like this" for the detail panel (Similar Titles switch), ranked by the
// same similarity function as Recommended for you.
import { useEffect, useState } from "react";
import type { JSX } from "react";
import { Layers } from "lucide-react";

import { isFeatureOn } from "../../features/featureFlags";
import { fetchSimilar, type ShelfItem } from "../../services/discovery";
import { playMedia } from "../../services/playback";
import { useAppStore, type MediaItem } from "../../store/appStore";
import { canPlayMediaItem } from "../../utils/mediaPlaybackSafety";
import DiscoveryCard, { type DiscoverySkin } from "./DiscoveryCard";

export default function MoreLikeThis({
  item,
  skin,
  onSelect,
}: {
  item: MediaItem;
  skin: DiscoverySkin;
  onSelect: (item: MediaItem) => void;
}): JSX.Element | null {
  const enabled = useAppStore((state) => isFeatureOn(state.featureSettings, "similar_titles"));
  const addStatusMessage = useAppStore((state) => state.addStatusMessage);
  const [similar, setSimilar] = useState<ShelfItem[] | null>(null);
  const mediaId = item.id;

  useEffect(() => {
    setSimilar(null);
    if (!enabled || mediaId == null) return;
    let active = true;
    fetchSimilar(mediaId, 8)
      .then((items) => { if (active) setSimilar(items); })
      .catch((error) => {
        if (active) setSimilar([]);
        addStatusMessage(`More like this unavailable: ${String(error)}`);
      });
    return () => { active = false; };
  }, [enabled, mediaId, addStatusMessage]);

  if (!enabled || mediaId == null) return null;

  const play = (entry: ShelfItem) => {
    if (!canPlayMediaItem(entry.item)) {
      addStatusMessage(`${entry.item.title} is not playable`);
      return;
    }
    playMedia([entry.item], 0, { source: "similar_titles" }).catch((error) =>
      addStatusMessage(`Playback failed: ${String(error)}`),
    );
  };

  return (
    <section className={`cv-more-like is-${skin}`} aria-label={`More like ${item.title}`}>
      <h4><Layers size={13} /> More like this</h4>
      {similar === null ? (
        <p className="cv-more-like__empty">Finding similar titles…</p>
      ) : similar.length === 0 ? (
        <p className="cv-more-like__empty">No similar titles in your library yet. Genres drive this list.</p>
      ) : (
        <div className="cv-more-like__grid">
          {similar.map((entry) => (
            <DiscoveryCard key={entry.item.work_key ?? entry.item.id} entry={entry} skin={skin} onSelect={onSelect} onPlay={play} compact />
          ))}
        </div>
      )}
    </section>
  );
}
