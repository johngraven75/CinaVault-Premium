// Server caching: artwork memory cache size and library JSON lifetime.
import { CACHE_DEFAULTS } from "../serverAdminLogic.ts";
import type { FeaturePanel } from "./types";

type CacheConfig = { artworkCacheMb?: number; libraryMaxAge?: number };

const clampInt = (text: string, min: number, max: number) => {
  const value = Math.round(Number(text));
  return Number.isFinite(value) ? Math.min(max, Math.max(min, value)) : null;
};

const CdnCachePanel: FeaturePanel<CacheConfig> = ({ config, setConfig }) => {
  const artwork = config.artworkCacheMb ?? CACHE_DEFAULTS.artworkCacheMb;
  const maxAge = config.libraryMaxAge ?? CACHE_DEFAULTS.libraryMaxAge;
  return (
    <div className="space-y-1.5">
      <label className="flex items-center gap-2 text-xs">
        Artwork kept in memory up to
        <input
          key={`a${artwork}`}
          className="cv-input w-20 text-xs"
          inputMode="numeric"
          defaultValue={artwork}
          aria-label="Artwork cache size in MB"
          onBlur={(event) => {
            const value = clampInt(event.target.value, 0, 2048);
            if (value !== null && value !== artwork) void setConfig({ artworkCacheMb: value });
          }}
        />
        MB
      </label>
      <label className="flex items-center gap-2 text-xs">
        Clients may reuse library lists for
        <input
          key={`l${maxAge}`}
          className="cv-input w-20 text-xs"
          inputMode="numeric"
          defaultValue={maxAge}
          aria-label="Library cache lifetime in seconds"
          onBlur={(event) => {
            const value = clampInt(event.target.value, 0, 3600);
            if (value !== null && value !== maxAge) void setConfig({ libraryMaxAge: value });
          }}
        />
        seconds
      </label>
      <p className="text-[10px] text-cv-subtext">
        Artwork is sent with a one-day immutable cache and ETags; sign-in responses are never cached.
      </p>
    </div>
  );
};

export default CdnCachePanel;
