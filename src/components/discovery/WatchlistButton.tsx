// Add/remove toggle for the active profile's watchlist. Renders nothing while
// the Watchlist switch is off.
import type { JSX, MouseEvent } from "react";
import { Bookmark, BookmarkCheck } from "lucide-react";

import { isFeatureOn } from "../../features/featureFlags";
import { isOnWatchlist } from "../../services/discovery";
import { useAppStore, type MediaItem } from "../../store/appStore";
import { useDiscoveryStore } from "./discoveryStore";

export default function WatchlistButton({
  item,
  className = "",
  compact = false,
}: {
  item: MediaItem;
  className?: string;
  /** Icon only (cards); the label stays available to screen readers. */
  compact?: boolean;
}): JSX.Element | null {
  const enabled = useAppStore((state) => isFeatureOn(state.featureSettings, "watchlist"));
  const addStatusMessage = useAppStore((state) => state.addStatusMessage);
  const watchlistIds = useDiscoveryStore((state) => state.watchlistIds);
  const toggle = useDiscoveryStore((state) => state.toggle);
  if (!enabled || item.id == null) return null;

  const saved = isOnWatchlist(item, watchlistIds);
  const label = saved ? "Remove from Watchlist" : "Add to Watchlist";
  const Icon = saved ? BookmarkCheck : Bookmark;

  const onClick = (event: MouseEvent) => {
    event.stopPropagation();
    toggle(item)
      .then((now) => addStatusMessage(now ? `Added to Watchlist: ${item.title}` : `Removed from Watchlist: ${item.title}`))
      .catch((error) => addStatusMessage(`Watchlist update failed: ${String(error)}`));
  };

  return (
    <button
      type="button"
      onClick={onClick}
      className={`cv-watchlist-btn ${saved ? "is-saved" : ""} ${className}`}
      aria-pressed={saved}
      aria-label={`${label}: ${item.title}`}
      title={label}
    >
      <Icon size={compact ? 12 : 14} />
      {compact ? <span className="sr-only">{label}</span> : <span>{saved ? "On Watchlist" : "Watchlist"}</span>}
    </button>
  );
}
