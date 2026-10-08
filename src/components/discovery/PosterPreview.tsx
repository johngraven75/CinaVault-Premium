// Synopsis preview shown while a poster is hovered or focused (Poster Hover
// Preview switch). Carries data-poster-preview so html.cv-no-poster-hover
// also hides it if a stale render survives the switch being turned off.
import type { JSX } from "react";

import { isFeatureOn } from "../../features/featureFlags";
import { previewText } from "../../services/discovery";
import { useAppStore, type MediaItem } from "../../store/appStore";

export default function PosterPreview({ item, max = 220 }: { item: MediaItem; max?: number }): JSX.Element | null {
  const enabled = useAppStore((state) => isFeatureOn(state.featureSettings, "poster_hover"));
  if (!enabled) return null;
  const synopsis = previewText(item.overview, max);
  const meta = [item.year, item.genre, item.rating ? `★ ${item.rating.toFixed(1)}` : null].filter(Boolean).join(" · ");
  return (
    <div className="cv-poster-preview" data-poster-preview="true" aria-hidden="true">
      {meta && <span className="cv-poster-preview__meta">{meta}</span>}
      <p>{synopsis || "No synopsis yet. Check Metadata to fetch one."}</p>
    </div>
  );
}
