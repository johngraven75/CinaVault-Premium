// A poster card for the discovery shelves, styled for the default (holo) or
// Kodi skin. Shows why the title is on the shelf, a progress bar for Continue
// Watching, the watchlist toggle and the hover synopsis preview.
import { useEffect, useState } from "react";
import type { JSX } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { Film, Play } from "lucide-react";

import type { MediaItem } from "../../store/appStore";
import type { ShelfItem } from "../../services/discovery";
import PosterPreview from "./PosterPreview";
import WatchlistButton from "./WatchlistButton";

export type DiscoverySkin = "holo" | "kodi";

/** Poster from a URL, a data URL, or a local file read through get_poster_data_url. */
export function DiscoveryPoster({ item, className = "" }: { item: MediaItem; className?: string }): JSX.Element {
  const path = item.poster_path || item.backdrop_path || null;
  const direct = path && /^(https?:|data:|asset:)/i.test(path) ? path : undefined;
  const [src, setSrc] = useState<string | undefined>(direct);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let active = true;
    setFailed(false);
    if (!path || direct) {
      setSrc(direct);
      return () => { active = false; };
    }
    setSrc(undefined);
    invoke<string>("get_poster_data_url", { path })
      .then((value) => { if (active) setSrc(value); })
      .catch(() => {
        if (!active) return;
        try {
          setSrc(convertFileSrc(path));
        } catch {
          setFailed(true);
        }
      });
    return () => { active = false; };
  }, [path, direct]);

  if (!src || failed) {
    return (
      <div className={`cv-disc-poster-fallback ${className}`} data-poster-fallback="true">
        <Film size={26} />
      </div>
    );
  }
  return <img src={src} alt="" className={className} loading="lazy" decoding="async" draggable={false} onError={() => setFailed(true)} />;
}

export default function DiscoveryCard({
  entry,
  skin,
  onSelect,
  onPlay,
  compact = false,
}: {
  entry: ShelfItem;
  skin: DiscoverySkin;
  onSelect: (item: MediaItem) => void;
  onPlay: (entry: ShelfItem) => void;
  compact?: boolean;
}): JSX.Element {
  const { item, reason, progress } = entry;
  const meta = [item.year, item.media_type].filter(Boolean).join(" · ");
  return (
    <article
      className={`cv-disc-card is-${skin} ${compact ? "is-compact" : ""}`}
      tabIndex={0}
      aria-label={`${item.title}${item.year ? ` (${item.year})` : ""}${reason ? `. ${reason}` : ""}`}
      onClick={() => onSelect(item)}
      onKeyDown={(event) => {
        if (event.target !== event.currentTarget) return;
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onSelect(item);
        }
      }}
    >
      <div className="cv-disc-poster">
        <DiscoveryPoster item={item} className="cv-disc-poster-img" />
        {typeof progress === "number" && (
          <div
            className="cv-disc-progress"
            role="progressbar"
            aria-label="Watched"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round(progress * 100)}
          >
            <span style={{ width: `${Math.max(2, progress * 100)}%` }} />
          </div>
        )}
        {!compact && <PosterPreview item={item} max={160} />}
        <div className="cv-disc-actions">
          <button
            type="button"
            className="cv-disc-play"
            onClick={(event) => {
              event.stopPropagation();
              onPlay(entry);
            }}
            aria-label={`${entry.resumeAt ? "Resume" : "Play"} ${item.title}`}
          >
            <Play size={13} fill="currentColor" /> {entry.resumeAt ? "Resume" : "Play"}
          </button>
          <WatchlistButton item={item} compact className="cv-disc-icon-btn" />
        </div>
      </div>
      <div className="cv-disc-info">
        <h4 className="cv-disc-title" title={item.title}>{item.title}</h4>
        {meta && <p className="cv-disc-meta">{meta}</p>}
        {reason && <p className="cv-disc-reason" title={reason}>{reason}</p>}
      </div>
    </article>
  );
}
