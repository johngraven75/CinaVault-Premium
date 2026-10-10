// Discovery shelves for the home screen (both skins): Continue Watching,
// New since your last visit, Trending, Recommended for you, Watchlist and the
// Genre Radio control. Each shelf exists only while its switch is on, follows
// the active profile and reloads on "cinavault:library-refresh".
import { useCallback, useEffect, useRef, useState } from "react";
import type { JSX, Ref } from "react";
import { Bell, Bookmark, Flame, History, Sparkles, X, type LucideIcon } from "lucide-react";

import { isFeatureOn, useFeature } from "../../features/featureFlags";
import {
  activeProfile,
  fetchContinueWatching,
  fetchNewReleases,
  fetchRecommendations,
  fetchTrending,
  newReleasesMessage,
  RECOMMENDATIONS_DEFAULTS,
  shelfLimit,
  TRENDING_DEFAULTS,
  type RecommendationsConfig,
  type ShelfItem,
  type TrendingConfig,
} from "../../services/discovery";
import { playMedia } from "../../services/playback";
import { useAppStore, type MediaItem } from "../../store/appStore";
import { canPlayMediaItem } from "../../utils/mediaPlaybackSafety";
import DiscoveryCard, { type DiscoverySkin } from "./DiscoveryCard";
import GenreRadio from "./GenreRadio";
import ShelfRow from "./ShelfRow";
import { useDiscoveryStore } from "./discoveryStore";

/** Profile id -> the "last visit" this session counts new titles from. */
const newReleaseBaselines = new Map<number, string>();

interface TrendingState {
  title: string;
  note: string | null;
  items: ShelfItem[];
}

interface NewReleasesState {
  count: number;
  items: ShelfItem[];
}

export default function DiscoveryShelves({
  skin,
  onSelect,
}: {
  skin: DiscoverySkin;
  onSelect: (item: MediaItem) => void;
}): JSX.Element | null {
  const featureSettings = useAppStore((state) => state.featureSettings);
  const addStatusMessage = useAppStore((state) => state.addStatusMessage);
  const continueOn = isFeatureOn(featureSettings, "continue_watching");
  const watchlistOn = isFeatureOn(featureSettings, "watchlist");
  const newReleasesOn = isFeatureOn(featureSettings, "new_releases");
  const trending = useFeature<TrendingConfig>("trending", TRENDING_DEFAULTS);
  const recommendations = useFeature<RecommendationsConfig>("recommendations", RECOMMENDATIONS_DEFAULTS);
  const trendingLimit = shelfLimit(trending.config.limit, TRENDING_DEFAULTS.limit);
  const trendingLocal = trending.config.source === "local";
  const recommendationLimit = shelfLimit(recommendations.config.limit, RECOMMENDATIONS_DEFAULTS.limit);

  const watchlist = useDiscoveryStore((state) => state.watchlist);
  const loadWatchlist = useDiscoveryStore((state) => state.loadWatchlist);
  const revision = useDiscoveryStore((state) => state.revision);

  const [reloadKey, setReloadKey] = useState(0);
  const [continueItems, setContinueItems] = useState<ShelfItem[]>([]);
  const [trendingShelf, setTrendingShelf] = useState<TrendingState | null>(null);
  const [recommended, setRecommended] = useState<ShelfItem[]>([]);
  const [newReleases, setNewReleases] = useState<NewReleasesState | null>(null);
  const [bannerDismissed, setBannerDismissed] = useState(false);
  const newShelfRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    const reload = () => setReloadKey((key) => key + 1);
    window.addEventListener("cinavault:library-refresh", reload);
    window.addEventListener("cinavault:profile-changed", reload);
    return () => {
      window.removeEventListener("cinavault:library-refresh", reload);
      window.removeEventListener("cinavault:profile-changed", reload);
    };
  }, []);

  const report = useCallback(
    (shelf: string) => (error: unknown) => addStatusMessage(`${shelf} shelf unavailable: ${String(error)}`),
    [addStatusMessage],
  );

  useEffect(() => {
    if (!continueOn) return setContinueItems([]);
    let active = true;
    fetchContinueWatching(24)
      .then((items) => { if (active) setContinueItems(items); })
      .catch(report("Continue Watching"));
    return () => { active = false; };
  }, [continueOn, reloadKey, report]);

  useEffect(() => {
    if (!watchlistOn) return;
    loadWatchlist().catch(report("Watchlist"));
  }, [watchlistOn, reloadKey, loadWatchlist, report]);

  useEffect(() => {
    if (!trending.enabled) return setTrendingShelf(null);
    let active = true;
    fetchTrending(trendingLimit, trendingLocal)
      .then((result) => {
        if (active && result.source !== "off") setTrendingShelf({ title: result.title, note: result.note, items: result.items });
      })
      .catch(report("Trending"));
    return () => { active = false; };
  }, [trending.enabled, trendingLimit, trendingLocal, reloadKey, report]);

  useEffect(() => {
    if (!recommendations.enabled) return setRecommended([]);
    let active = true;
    fetchRecommendations(recommendationLimit)
      .then((items) => { if (active) setRecommended(items); })
      .catch(report("Recommended"));
    return () => { active = false; };
  }, [recommendations.enabled, recommendationLimit, reloadKey, revision, report]);

  useEffect(() => {
    if (!newReleasesOn) return setNewReleases(null);
    let active = true;
    (async () => {
      const profile = await activeProfile();
      const baseline = newReleaseBaselines.get(profile.id);
      // First look this session: count from the stored last visit and record
      // this visit; later reloads keep counting from the same point.
      const result = await fetchNewReleases(baseline ?? null, baseline === undefined);
      if (baseline === undefined) {
        newReleaseBaselines.set(profile.id, result.since ?? result.checked_at);
        if (result.count > 0) addStatusMessage(newReleasesMessage(result.count));
      }
      if (active) setNewReleases({ count: result.count, items: result.items });
    })().catch(report("New Releases"));
    return () => { active = false; };
  }, [newReleasesOn, reloadKey, addStatusMessage, report]);

  const playFrom = useCallback(
    (shelf: string, items: ShelfItem[]) => (entry: ShelfItem) => {
      const item = entry.item;
      if (!canPlayMediaItem(item)) {
        addStatusMessage(`${item.title} is not playable`);
        return;
      }
      const options = { source: shelf, startAt: entry.resumeAt };
      // Continue Watching resumes one title; other shelves queue the rest of the shelf.
      const queue = entry.resumeAt ? [item] : items.map((other) => other.item).filter(canPlayMediaItem);
      const index = Math.max(0, queue.findIndex((other) => other.file_path === item.file_path));
      playMedia(queue.length ? queue : [item], index, options)
        .then(() => addStatusMessage(`${entry.resumeAt ? "Resuming" : "Playing"} ${item.title}`))
        .catch((error) => addStatusMessage(`Playback failed: ${String(error)}`));
    },
    [addStatusMessage],
  );

  const watchlistItems: ShelfItem[] = watchlistOn ? watchlist.map((item) => ({ item })) : [];
  const showBanner = newReleasesOn && !bannerDismissed && (newReleases?.count ?? 0) > 0;
  const anything =
    continueOn || watchlistOn || newReleasesOn || trending.enabled || recommendations.enabled ||
    isFeatureOn(featureSettings, "genre_radio");
  if (!anything) return null;

  return (
    <div className={`cv-discovery is-${skin}`}>
      {showBanner && newReleases && (
        <div className="cv-new-banner" role="status">
          <Bell size={15} />
          <span>{newReleasesMessage(newReleases.count)}</span>
          <button type="button" onClick={() => newShelfRef.current?.scrollIntoView({ behavior: "smooth", block: "start" })}>
            Show
          </button>
          <button type="button" onClick={() => setBannerDismissed(true)} aria-label="Dismiss new titles notice">
            <X size={14} />
          </button>
        </div>
      )}

      <GenreRadio skin={skin} reloadKey={reloadKey} />

      {continueOn && continueItems.length > 0 && (
        <Shelf title="Continue Watching" icon={History} items={continueItems} skin={skin} onSelect={onSelect} onPlay={playFrom("continue_watching", continueItems)} />
      )}
      {newReleasesOn && newReleases && newReleases.items.length > 0 && (
        <Shelf
          ref={newShelfRef}
          title="New since your last visit"
          icon={Bell}
          items={newReleases.items}
          count={newReleases.count}
          skin={skin}
          onSelect={onSelect}
          onPlay={playFrom("new_releases", newReleases.items)}
        />
      )}
      {trending.enabled && trendingShelf && (
        <Shelf
          title={trendingShelf.title}
          icon={Flame}
          items={trendingShelf.items}
          note={trendingShelf.note}
          empty="Nothing played in the last 30 days yet. Titles you play will show up here."
          skin={skin}
          onSelect={onSelect}
          onPlay={playFrom("trending", trendingShelf.items)}
        />
      )}
      {recommendations.enabled && (
        <Shelf
          title="Recommended for you"
          icon={Sparkles}
          items={recommended}
          empty="Watch, favorite or save a few titles and recommendations will appear here."
          skin={skin}
          onSelect={onSelect}
          onPlay={playFrom("recommendations", recommended)}
        />
      )}
      {watchlistOn && (
        <Shelf
          title="Watchlist"
          icon={Bookmark}
          items={watchlistItems}
          empty="Use the bookmark button on any title to save it here."
          skin={skin}
          onSelect={onSelect}
          onPlay={playFrom("watchlist", watchlistItems)}
        />
      )}
    </div>
  );
}

interface ShelfProps {
  title: string;
  icon: LucideIcon;
  items: ShelfItem[];
  skin: DiscoverySkin;
  onSelect: (item: MediaItem) => void;
  onPlay: (entry: ShelfItem) => void;
  count?: number;
  note?: string | null;
  empty?: string;
  ref?: Ref<HTMLElement>;
}

function Shelf({ title, icon: Icon, items, skin, onSelect, onPlay, count, note, empty, ref }: ShelfProps): JSX.Element {
  return (
    <section ref={ref} className={`cv-disc-shelf is-${skin}`} aria-label={title}>
      <header className="cv-disc-shelf__head">
        <h3>
          <Icon size={15} /> {title}
          {items.length > 0 && <span className="cv-disc-shelf__count">{count ?? items.length}</span>}
        </h3>
        {note && <p className="cv-disc-shelf__note">{note}</p>}
      </header>
      {items.length ? (
        <ShelfRow label={title}>
          {items.map((entry) => (
            <DiscoveryCard key={`${entry.item.work_key ?? entry.item.id ?? entry.item.file_path}`} entry={entry} skin={skin} onSelect={onSelect} onPlay={onPlay} />
          ))}
        </ShelfRow>
      ) : (
        <p className="cv-disc-shelf__empty">{empty}</p>
      )}
    </section>
  );
}
