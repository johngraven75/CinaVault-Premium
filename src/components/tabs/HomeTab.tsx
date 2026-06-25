// CinaVault Premium — Build 137 Hyper-Neon Fusion Library HUD
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { JSX, PointerEvent } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { AnimatePresence, motion } from "framer-motion";
import { useAppStore, MediaItem } from "../../store/appStore";
import {
  filterItemsByTitleInitial,
  TITLE_LETTERS,
  type TitleInitialFilter,
} from "../../utils/libraryAlphabetFilter";
import {
  buildLibraryPageRequest,
  hasMoreLibraryPages,
  LIBRARY_PAGE_SIZE,
  mergeLibraryPage,
  shouldAutoLoadNextLibraryPage,
} from "../../utils/libraryLoadPolicy";
import { canPlayMediaItem, isLibraryDisplayableMediaItem } from "../../utils/mediaPlaybackSafety";
import MeteorShower from "../effects/MeteorShower";
import {
  Activity,
  ChevronDown,
  CheckCircle,
  Clock,
  Database,
  Disc3,
  Film,
  Grid3X3,
  Heart,
  List,
  PanelTop,
  Play,
  RectangleHorizontal,
  RefreshCw,
  RotateCw,
  Search,
  Sparkles,
  Star,
  X,
  Zap,
  type LucideIcon,
} from "lucide-react";

type Shelf = "recent" | "verified" | "unverified" | "favorites";
type CardStyle = "poster" | "disc" | "banner";

interface ShelfOption {
  id: Shelf;
  label: string;
  icon: LucideIcon;
}

const SHELF_OPTIONS: ShelfOption[] = [
  { id: "recent", label: "Trending Now", icon: Clock },
  { id: "verified", label: "Verified Signal", icon: CheckCircle },
  { id: "unverified", label: "Needs Metadata", icon: Sparkles },
  { id: "favorites", label: "My Vault", icon: Heart },
];

function resolveMediaImageSrc(path?: string | null): string | undefined {
  if (!path) return undefined;
  if (/^(https?:|data:|asset:)/i.test(path)) return path;
  try {
    return convertFileSrc(path);
  } catch {
    return path;
  }
}

function formatRuntime(minutes?: number): string {
  if (!minutes) return "N/A";
  const hours = Math.floor(minutes / 60);
  const mins = Math.round(minutes % 60);
  return hours > 0 ? `${hours}h ${mins}m` : `${mins}m`;
}

function calculateWatchtimeHours(items: MediaItem[]): number {
  const minutes = items.reduce((total, item) => total + (item.duration || 0), 0);
  return Math.max(0, Math.round(minutes / 60));
}

function sortRecent(items: MediaItem[]): MediaItem[] {
  return [...items].sort((a, b) => {
    const aTime = Date.parse(a.date_added || "") || 0;
    const bTime = Date.parse(b.date_added || "") || 0;
    return bTime - aTime;
  });
}

export default function HomeTab(): JSX.Element {
  const {
    mediaItems,
    setMediaItems,
    selectedMedia,
    setSelectedMedia,
    libraryView,
    setLibraryView,
    searchQuery,
    addStatusMessage,
  } = useAppStore();

  const [activeShelf, setActiveShelf] = useState<Shelf>("recent");
  const [typeFilter, setTypeFilter] = useState<string>("all");
  const [loading, setLoading] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [autoLoadingLibrary, setAutoLoadingLibrary] = useState(false);
  const [libraryOffset, setLibraryOffset] = useState(0);
  const [libraryHasMore, setLibraryHasMore] = useState(false);
  const [iconSize, setIconSize] = useState(150);
  const [cardStyle, setCardStyle] = useState<CardStyle>("poster");
  const [detailFlipped, setDetailFlipped] = useState(false);
  const [titleInitialFilter, setTitleInitialFilter] = useState<TitleInitialFilter>("all");
  const [metadataCheckId, setMetadataCheckId] = useState<number | null>(null);
  const filterListRef = useRef<HTMLDivElement | null>(null);
  const libraryLoadGenerationRef = useRef(0);

  const requestMediaPage = useCallback((offset: number) => {
    return invoke<MediaItem[]>(
      "get_media_items",
      buildLibraryPageRequest({ mediaType: typeFilter, offset }),
    );
  }, [typeFilter]);

  const applyUpdatedMediaItem = useCallback((updated: Partial<MediaItem> & { id?: number }) => {
    if (!updated.id) return;
    setMediaItems(mediaItems.map((item) => (item.id === updated.id ? { ...item, ...updated } : item)));
    if (selectedMedia?.id === updated.id) {
      setSelectedMedia({ ...selectedMedia, ...updated });
    }
  }, [mediaItems, selectedMedia, setMediaItems, setSelectedMedia]);

  const loadMedia = useCallback(async () => {
    const generation = libraryLoadGenerationRef.current + 1;
    libraryLoadGenerationRef.current = generation;
    setLoading(true);
    setAutoLoadingLibrary(false);

    try {
      const items = await requestMediaPage(0);
      if (generation !== libraryLoadGenerationRef.current) return;
      const hasMore = hasMoreLibraryPages(items);
      setMediaItems(items);
      setLibraryOffset(items.length);
      setLibraryHasMore(hasMore);
      setAutoLoadingLibrary(shouldAutoLoadNextLibraryPage(items));
      addStatusMessage(
        hasMore
          ? `HUD core opened ${items.length} vault records; compiling the full library in the background`
          : `HUD core loaded ${items.length} vault records`,
      );
    } catch {
      if (generation !== libraryLoadGenerationRef.current) return;
      setMediaItems(DEMO_ITEMS);
      setLibraryOffset(DEMO_ITEMS.length);
      setLibraryHasMore(false);
      setAutoLoadingLibrary(false);
      addStatusMessage("HUD demo records loaded while backend media bridge is unavailable");
    } finally {
      if (generation === libraryLoadGenerationRef.current) {
        setLoading(false);
      }
    }
  }, [addStatusMessage, requestMediaPage, setMediaItems]);

  const loadMoreMedia = useCallback(async (automatic = false) => {
    if (loading || loadingMore || !libraryHasMore) return;
    const generation = libraryLoadGenerationRef.current;
    setLoadingMore(true);

    try {
      const items = await requestMediaPage(libraryOffset);
      if (generation !== libraryLoadGenerationRef.current) return;
      const mergedItems = mergeLibraryPage(mediaItems, items);
      const hasMore = hasMoreLibraryPages(items);
      setMediaItems(mergedItems);
      setLibraryOffset(libraryOffset + items.length);
      setLibraryHasMore(hasMore);
      setAutoLoadingLibrary(automatic && shouldAutoLoadNextLibraryPage(items));
      if (!hasMore) {
        addStatusMessage(`HUD library compile complete: ${mergedItems.length} vault records online`);
      } else if (!automatic) {
        addStatusMessage(`Compiled ${items.length} more records (${mergedItems.length} online)`);
      }
    } catch (error) {
      setAutoLoadingLibrary(false);
      addStatusMessage(`HUD compile failed: ${error}`);
    } finally {
      setLoadingMore(false);
    }
  }, [
    addStatusMessage,
    libraryHasMore,
    libraryOffset,
    loading,
    loadingMore,
    mediaItems,
    requestMediaPage,
    setMediaItems,
  ]);

  useEffect(() => {
    void loadMedia();
  }, [loadMedia]);

  useEffect(() => {
    if (!autoLoadingLibrary || loading || loadingMore || !libraryHasMore) return;
    const timer = window.setTimeout(() => void loadMoreMedia(true), 0);
    return () => window.clearTimeout(timer);
  }, [autoLoadingLibrary, libraryHasMore, loading, loadingMore, loadMoreMedia]);

  useEffect(() => {
    setDetailFlipped(false);
  }, [selectedMedia?.id, selectedMedia?.title]);

  useEffect(() => {
    const activeTabButton = filterListRef.current?.querySelector<HTMLButtonElement>(".alphabet-filter-button.active");
    activeTabButton?.scrollIntoView({ behavior: "smooth", inline: "center", block: "nearest" });
  }, [titleInitialFilter]);

  const filteredItems = useMemo(() => {
    let items = mediaItems.filter(isLibraryDisplayableMediaItem);
    if (searchQuery) {
      const q = searchQuery.toLowerCase();
      items = items.filter((item) => (
        item.title.toLowerCase().includes(q)
        || item.genre?.toLowerCase().includes(q)
        || item.resolution?.toLowerCase().includes(q)
        || item.codec?.toLowerCase().includes(q)
      ));
    }
    if (typeFilter !== "all") items = items.filter((item) => item.media_type === typeFilter);
    switch (activeShelf) {
      case "verified":
        items = items.filter((item) => item.verified);
        break;
      case "unverified":
        items = items.filter((item) => !item.verified);
        break;
      case "favorites":
        items = items.filter((item) => item.favorite);
        break;
      default:
        items = sortRecent(items);
        break;
    }
    return filterItemsByTitleInitial(items, titleInitialFilter);
  }, [activeShelf, mediaItems, searchQuery, titleInitialFilter, typeFilter]);

  const heroItem = selectedMedia || filteredItems[0] || DEMO_ITEMS[0];
  const trendingItems = useMemo(() => filteredItems.slice(0, 8), [filteredItems]);
  const watchtimeHours = useMemo(() => calculateWatchtimeHours(mediaItems), [mediaItems]);
  const verifiedCount = filteredItems.filter((item) => item.verified).length;
  const movieCount = filteredItems.filter((item) => item.media_type === "movie").length;
  const cardMinWidth = cardStyle === "banner" ? Math.max(260, Math.round(iconSize * 1.72)) : Math.max(118, iconSize);

  const handlePlay = async (item: MediaItem): Promise<void> => {
    if (!canPlayMediaItem(item)) {
      addStatusMessage(`Quick Play skipped: ${item.title} is not a playable video or audio record`);
      return;
    }

    try {
      await invoke("play_media", { filePath: item.file_path });
      addStatusMessage(`Quick Play engaged: ${item.title}`);
    } catch (error) {
      addStatusMessage(`Quick Play failed: ${error}`);
    }
  };

  const handleVerify = async (item: MediaItem): Promise<void> => {
    try {
      await invoke("verify_media_item", { id: item.id });
      addStatusMessage(`Verification pulse complete: ${item.title}`);
      await loadMedia();
    } catch (error) {
      addStatusMessage(`Verification pulse failed: ${error}`);
    }
  };

  const handleCheckMetadata = async (item: MediaItem): Promise<void> => {
    if (!item.id) {
      addStatusMessage(`Metadata scan skipped: ${item.title} has no vault id yet`);
      return;
    }

    setMetadataCheckId(item.id);
    try {
      const result = await invoke<any>("check_media_item_metadata", { id: item.id });
      if (result?.updated_item) {
        applyUpdatedMediaItem(result.updated_item);
      }
      addStatusMessage(result?.message || `Metadata scan complete: ${item.title}`);
    } catch (error) {
      addStatusMessage(`Metadata scan failed for ${item.title}: ${error}`);
    } finally {
      setMetadataCheckId(null);
    }
  };

  return (
    <div className="cyber-home space-y-5">
      <section className="cyber-hero">
        <MeteorShower meteorCount={34} />
        {resolveMediaImageSrc(heroItem.backdrop_path || heroItem.poster_path) && (
          <div
            className="absolute inset-0 z-0 opacity-30"
            style={{
              backgroundImage: `url(${resolveMediaImageSrc(heroItem.backdrop_path || heroItem.poster_path)})`,
              backgroundSize: "cover",
              backgroundPosition: "center",
            }}
          />
        )}
        <div className="absolute inset-0 z-[1] bg-[linear-gradient(90deg,rgba(5,5,10,0.95),rgba(5,5,10,0.52)_45%,rgba(5,5,10,0.82)),radial-gradient(circle_at_80%_22%,rgba(189,0,255,0.24),transparent_36%)]" />

        <div className="relative z-10 grid min-h-[310px] gap-5 p-5 lg:grid-cols-[minmax(0,1fr)_320px]">
          <div className="flex min-w-0 flex-col justify-end">
            <div className="cyber-eyebrow mb-2 flex items-center gap-2">
              <Zap size={14} /> Trending Now / Holographic Carousel
            </div>
            <AnimatePresence mode="wait">
              <motion.div
                key={heroItem.title}
                initial={{ opacity: 0, y: 18, filter: "blur(12px)" }}
                animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
                exit={{ opacity: 0, y: -10, filter: "blur(8px)" }}
                transition={{ duration: 0.32 }}
              >
                <h2 className="cyber-title max-w-4xl text-4xl font-black tracking-tight lg:text-6xl">{heroItem.title}</h2>
                <div className="mt-3 flex flex-wrap items-center gap-2">
                  {heroItem.year && <span className="cyber-chip">{heroItem.year}</span>}
                  <span className="cyber-chip">{heroItem.media_type || "media"}</span>
                  {heroItem.resolution && <span className="cyber-chip">{heroItem.resolution}</span>}
                  {heroItem.rating && (
                    <span className="cyber-chip is-hot"><Star size={12} /> {heroItem.rating}</span>
                  )}
                </div>
                {heroItem.overview && (
                  <p className="mt-4 max-w-3xl text-sm leading-7 text-cv-subtext">{heroItem.overview}</p>
                )}
              </motion.div>
            </AnimatePresence>

            <div className="mt-5 flex flex-wrap gap-2">
              <button type="button" onClick={() => void handlePlay(heroItem)} className="cyber-button">
                <Play size={15} /> Quick Play
              </button>
              <button type="button" onClick={() => setSelectedMedia(heroItem)} className="cyber-button">
                <Sparkles size={15} /> Open Terminal Panel
              </button>
              <button type="button" onClick={() => void handleCheckMetadata(heroItem)} className="cyber-button is-amber">
                <Search size={15} /> Parse Metadata
              </button>
            </div>
          </div>

          <div className="cyber-terminal-panel hidden bg-black/35 p-4 lg:block">
            <div className="cyber-eyebrow mb-3 flex items-center gap-2"><Activity size={13} /> User Terminal Quick-Stats</div>
            <TerminalLine label="Watchtime" value={`${watchtimeHours}h`} />
            <TerminalLine label="Vault Inventory" value={mediaItems.length.toLocaleString()} />
            <TerminalLine label="Visible Records" value={filteredItems.length.toLocaleString()} />
            <TerminalLine label="Verified Signal" value={`${verifiedCount} locked`} />
            <TerminalLine label="System Status" value={autoLoadingLibrary ? "Compiling" : "Nominal"} />
          </div>
        </div>
      </section>

      <section className="grid gap-3 md:grid-cols-3">
        <StatCard icon={Clock} label="Watchtime" value={`${watchtimeHours}h`} detail="Library runtime index" />
        <StatCard icon={Database} label="Vault Inventory" value={mediaItems.length.toLocaleString()} detail={`${filteredItems.length} visible records`} />
        <StatCard icon={Activity} label="System Status" value={autoLoadingLibrary ? "Compiling" : "Nominal"} detail={`${verifiedCount} verified / ${movieCount} movies`} />
      </section>

      <section className="cyber-control-core">
        <div className="relative z-10 flex flex-wrap items-center justify-between gap-3">
          <div className="flex flex-wrap gap-2">
            {SHELF_OPTIONS.map((shelf) => {
              const Icon = shelf.icon;
              const active = activeShelf === shelf.id;
              return (
                <button
                  key={shelf.id}
                  type="button"
                  onClick={() => setActiveShelf(shelf.id)}
                  className={`cyber-button text-xs ${active ? "is-amber" : ""}`}
                >
                  <Icon size={13} /> {shelf.label}
                </button>
              );
            })}
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <select value={typeFilter} onChange={(event) => setTypeFilter(event.target.value)} className="cyber-select">
              <option value="all">All Types</option>
              <option value="movie">Movies</option>
              <option value="tvshow">TV Shows</option>
              <option value="music">Music</option>
              <option value="photo">Photos</option>
            </select>

            <div className="flex overflow-hidden border border-cyan-300/20 bg-black/40">
              <button type="button" onClick={() => setLibraryView("card")} className={`cyber-button h-10 w-10 px-0 ${libraryView === "card" ? "is-amber" : ""}`} title="Card view">
                <Grid3X3 size={14} />
              </button>
              <button type="button" onClick={() => setLibraryView("list")} className={`cyber-button h-10 w-10 px-0 ${libraryView === "list" ? "is-amber" : ""}`} title="List view">
                <List size={14} />
              </button>
            </div>

            <button type="button" onClick={() => void loadMedia()} className="cyber-button text-xs">
              <RefreshCw size={13} className={loading ? "animate-spin" : ""} /> Refresh
            </button>
          </div>
        </div>

        {libraryView === "card" && (
          <div className="relative z-10 mt-4 grid gap-3 lg:grid-cols-[1fr_330px]">
            <div className="flex flex-wrap items-center gap-2">
              <LayoutButton active={cardStyle === "poster"} icon={PanelTop} label="Poster" onClick={() => setCardStyle("poster")} />
              <LayoutButton active={cardStyle === "disc"} icon={Disc3} label="Disc" onClick={() => setCardStyle("disc")} />
              <LayoutButton active={cardStyle === "banner"} icon={RectangleHorizontal} label="Banner" onClick={() => setCardStyle("banner")} />
            </div>

            <div className="flex items-center gap-3">
              <span className="cyber-stat-label w-24">Card Scale</span>
              <input
                type="range"
                min={96}
                max={224}
                step={4}
                value={iconSize}
                onChange={(event) => setIconSize(Number(event.target.value))}
                className="w-full accent-[var(--cyber-cyan)]"
              />
              <span className="w-10 text-right font-mono text-xs text-cv-text">{iconSize}</span>
            </div>
          </div>
        )}

        <div ref={filterListRef} className="relative z-10 mt-4 alphabet-filter" role="tablist" aria-label="Filter library by title initial" tabIndex={0}>
          {(["all", ...TITLE_LETTERS, "#"] as TitleInitialFilter[]).map((letter) => (
            <button
              key={letter}
              type="button"
              role="tab"
              aria-selected={titleInitialFilter === letter}
              onClick={() => setTitleInitialFilter(letter)}
              className={`alphabet-filter-button ${titleInitialFilter === letter ? "active" : ""}`}
              title={letter === "all" ? "Show all titles" : `Show ${letter} titles`}
            >
              {letter === "all" ? "All" : letter}
            </button>
          ))}
        </div>
      </section>

      {trendingItems.length > 0 && (
        <section className="cyber-panel rounded-[18px] p-4">
          <div className="relative z-10 mb-3 flex items-center justify-between gap-3">
            <div>
              <div className="cyber-eyebrow flex items-center gap-2"><Sparkles size={13} /> Holographic Carousel</div>
              <h3 className="text-lg font-black uppercase tracking-[0.12em] text-cv-text">Trending Data Blocks</h3>
            </div>
            <span className="cyber-chip">{trendingItems.length} indexed</span>
          </div>
          <div className="relative z-10 flex gap-3 overflow-x-auto pb-2">
            {trendingItems.map((item, index) => (
              <motion.button
                key={`${item.id || item.title}-trend-${index}`}
                type="button"
                onClick={() => setSelectedMedia(item)}
                className="cyber-card w-[156px] shrink-0 text-left"
                initial={{ opacity: 0, y: 16 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: Math.min(index * 0.035, 0.2) }}
              >
                <CardVisual item={item} styleMode="poster" />
                <div className="p-3">
                  <div className="truncate text-xs font-black text-cv-text">{item.title}</div>
                  <div className="mt-1 flex items-center gap-2 text-[10px] text-cv-subtext">
                    <span>{item.year || "—"}</span>
                    <span className="uppercase">{item.media_type}</span>
                  </div>
                </div>
              </motion.button>
            ))}
          </div>
        </section>
      )}

      <section className={`grid gap-4 ${selectedMedia ? "xl:grid-cols-[minmax(0,1fr)_360px]" : ""}`}>
        <div>
          {loading ? (
            <div className="cyber-grid" style={{ gridTemplateColumns: `repeat(auto-fill, minmax(${cardMinWidth}px, 1fr))` }}>
              {Array.from({ length: 12 }).map((_, index) => (
                <div key={index} className="cyber-card shimmer h-56" />
              ))}
            </div>
          ) : filteredItems.length === 0 ? (
            <div className="cyber-panel rounded-[18px] p-12 text-center">
              <Film size={48} className="mx-auto mb-4 text-cv-subtext/40" />
              <h3 className="text-lg font-black uppercase tracking-[0.12em] text-cv-text">No Media Found</h3>
              <p className="mt-2 text-sm text-cv-subtext">Add media sources and scan to populate the holographic vault.</p>
            </div>
          ) : libraryView === "card" ? (
            <motion.div
              key={`${searchQuery}-${activeShelf}-${typeFilter}-${titleInitialFilter}-${cardStyle}`}
              className="cyber-grid"
              style={{ gridTemplateColumns: `repeat(auto-fill, minmax(${cardMinWidth}px, 1fr))` }}
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              transition={{ duration: 0.2 }}
            >
              {filteredItems.map((item, index) => (
                <MediaCard
                  key={`${item.id || item.title}-${index}`}
                  item={item}
                  styleMode={cardStyle}
                  checking={metadataCheckId === item.id}
                  index={index}
                  onSelect={() => setSelectedMedia(item)}
                  onPlay={() => void handlePlay(item)}
                  onCheckMetadata={() => void handleCheckMetadata(item)}
                />
              ))}
            </motion.div>
          ) : (
            <div className="cyber-table">
              <div className="cyber-table-row cyber-stat-label bg-cyan-300/[0.06]">
                <span>Title</span><span>Type</span><span>Year</span><span>Rating</span><span>Status</span>
              </div>
              {filteredItems.map((item, index) => (
                <button
                  key={`${item.id || item.title}-row-${index}`}
                  type="button"
                  onClick={() => setSelectedMedia(item)}
                  className="cyber-table-row w-full text-left text-sm"
                >
                  <span className="truncate font-semibold">{item.title}</span>
                  <span className="text-xs capitalize text-cv-subtext">{item.media_type}</span>
                  <span className="text-xs text-cv-subtext">{item.year || "—"}</span>
                  <span className="flex items-center gap-1 text-xs">
                    {item.rating ? <><Star size={11} className="text-[var(--cyber-amber)]" />{item.rating}</> : "—"}
                  </span>
                  <span>{item.verified ? <CheckCircle size={15} className="text-cyan-200" /> : <Clock size={15} className="text-cv-subtext/50" />}</span>
                </button>
              ))}
            </div>
          )}
        </div>

        <AnimatePresence>
          {selectedMedia && (
            <motion.aside
              key={selectedMedia.id || selectedMedia.title}
              initial={{ opacity: 0, x: 28, filter: "blur(10px)" }}
              animate={{ opacity: 1, x: 0, filter: "blur(0px)" }}
              exit={{ opacity: 0, x: 28, filter: "blur(10px)" }}
              transition={{ duration: 0.24 }}
              className="cyber-terminal-panel bg-[#05050a]/90 p-4"
            >
              <div className="mb-4 flex items-start justify-between gap-3">
                <div>
                  <div className="cyber-eyebrow flex items-center gap-2"><Sparkles size={13} /> Terminal Panel</div>
                  <h3 className="mt-1 text-xl font-black leading-tight text-cv-text">{selectedMedia.title}</h3>
                </div>
                <button type="button" onClick={() => setSelectedMedia(null)} className="cyber-button h-10 w-10 px-0" title="Close terminal panel">
                  <X size={15} />
                </button>
              </div>

              <button type="button" onClick={() => setDetailFlipped((value) => !value)} className="detail-card-stage mb-4 w-full text-left" title="Rotate terminal containment card">
                <motion.div
                  className="detail-card-inner"
                  animate={{ rotateY: detailFlipped ? 180 : 0 }}
                  transition={{ duration: 0.62, ease: [0.16, 1, 0.3, 1] }}
                >
                  <div className="detail-face detail-face-front border border-cyan-300/25 bg-black/70">
                    {resolveMediaImageSrc(selectedMedia.poster_path) ? (
                      <img src={resolveMediaImageSrc(selectedMedia.poster_path)} alt={selectedMedia.title} className="h-full w-full object-cover opacity-90" />
                    ) : (
                      <div className="flex h-full w-full items-center justify-center"><Film size={46} className="text-cv-accent/60" /></div>
                    )}
                    <div className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/90 via-black/35 to-transparent p-3">
                      <div className="cyber-eyebrow mb-1 text-[9px]">Containment Unit</div>
                      <div className="truncate font-bold">{selectedMedia.title}</div>
                    </div>
                  </div>
                  <div className="detail-face detail-face-back border border-cyan-300/25 bg-gradient-to-br from-black to-cyan-950/30 p-4">
                    <div className="cyber-eyebrow mb-3">Media Telemetry</div>
                    <TerminalLine label="Type" value={selectedMedia.media_type || "Unknown"} />
                    <TerminalLine label="Resolution" value={selectedMedia.resolution || "N/A"} />
                    <TerminalLine label="Codec" value={selectedMedia.codec || "Auto"} />
                    <TerminalLine label="Runtime" value={formatRuntime(selectedMedia.duration)} />
                    <TerminalLine label="Rating" value={selectedMedia.rating ? `${selectedMedia.rating}` : "N/A"} />
                    <div className="mt-3 text-[11px] text-cv-subtext">Tap to rotate containment unit.</div>
                  </div>
                </motion.div>
              </button>

              <div className="space-y-1">
                <TerminalLine label="Year" value={selectedMedia.year ? `${selectedMedia.year}` : "N/A"} />
                <TerminalLine label="Genre" value={selectedMedia.genre || "Unclassified"} />
                <TerminalLine label="Verified" value={selectedMedia.verified ? "Locked" : "Pending"} />
                <TerminalLine label="Favorite" value={selectedMedia.favorite ? "Vaulted" : "Not Set"} />
              </div>

              {selectedMedia.overview && (
                <p className="mt-4 rounded border border-cyan-300/10 bg-black/30 p-3 text-xs leading-6 text-cv-subtext">{selectedMedia.overview}</p>
              )}

              <div className="mt-4 grid gap-2">
                <button type="button" onClick={() => void handlePlay(selectedMedia)} className="cyber-button">
                  <Play size={14} /> Quick Play
                </button>
                <button type="button" onClick={() => void handleVerify(selectedMedia)} className="cyber-button">
                  <CheckCircle size={14} /> Verify Signal
                </button>
                <button type="button" onClick={() => void handleCheckMetadata(selectedMedia)} disabled={metadataCheckId === selectedMedia.id} className="cyber-button is-amber disabled:opacity-60">
                  {metadataCheckId === selectedMedia.id ? <RefreshCw size={14} className="animate-spin" /> : <Sparkles size={14} />}
                  Parse Metadata
                </button>
                <button type="button" onClick={() => setDetailFlipped((value) => !value)} className="cyber-button">
                  <RotateCw size={14} /> Rotate Unit
                </button>
              </div>
            </motion.aside>
          )}
        </AnimatePresence>
      </section>

      {autoLoadingLibrary && !loading && (
        <div className="flex justify-center">
          <div className="cyber-button pointer-events-none">
            <RefreshCw size={14} className="animate-spin" /> Compiling full library ({mediaItems.length} online)
          </div>
        </div>
      )}

      {libraryHasMore && !loading && !autoLoadingLibrary && (
        <div className="flex justify-center">
          <button type="button" onClick={() => void loadMoreMedia()} disabled={loadingMore} className="cyber-button">
            {loadingMore ? <RefreshCw size={14} className="animate-spin" /> : <ChevronDown size={14} />}
            {loadingMore ? "Compiling" : `Load Next ${LIBRARY_PAGE_SIZE}`}
          </button>
        </div>
      )}
    </div>
  );
}

function LayoutButton({ active, icon: Icon, label, onClick }: { active: boolean; icon: LucideIcon; label: string; onClick: () => void }): JSX.Element {
  return (
    <button type="button" onClick={onClick} className={`cyber-button text-xs ${active ? "is-amber" : ""}`}>
      <Icon size={13} /> {label}
    </button>
  );
}

function StatCard({ icon: Icon, label, value, detail }: { icon: LucideIcon; label: string; value: string; detail: string }): JSX.Element {
  return (
    <div className="cyber-stat">
      <div className="relative z-10 flex items-start justify-between gap-3">
        <div>
          <div className="cyber-stat-label">{label}</div>
          <div className="cyber-stat-value mt-2">{value}</div>
          <div className="mt-2 text-xs text-cv-subtext">{detail}</div>
        </div>
        <div className="grid h-11 w-11 place-items-center border border-cyan-300/25 bg-cyan-300/10 text-cyan-200 shadow-[0_0_18px_rgba(0,245,255,0.16)]">
          <Icon size={18} />
        </div>
      </div>
    </div>
  );
}

function TerminalLine({ label, value }: { label: string; value: string }): JSX.Element {
  return (
    <div className="terminal-line">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function MediaCard({
  item,
  styleMode,
  checking,
  index,
  onSelect,
  onPlay,
  onCheckMetadata,
}: {
  item: MediaItem;
  styleMode: CardStyle;
  checking: boolean;
  index: number;
  onSelect: () => void;
  onPlay: () => void;
  onCheckMetadata: () => void;
}): JSX.Element {
  const handlePointerMove = (event: PointerEvent<HTMLDivElement>): void => {
    const rect = event.currentTarget.getBoundingClientRect();
    const x = (event.clientX - rect.left) / rect.width - 0.5;
    const y = (event.clientY - rect.top) / rect.height - 0.5;
    event.currentTarget.style.setProperty("--tilt-x", `${x * 8}deg`);
    event.currentTarget.style.setProperty("--tilt-y", `${y * -7}deg`);
  };

  const resetPointerTilt = (event: PointerEvent<HTMLDivElement>): void => {
    event.currentTarget.style.setProperty("--tilt-x", "0deg");
    event.currentTarget.style.setProperty("--tilt-y", "0deg");
  };

  return (
    <motion.div
      className="cyber-card group"
      initial={{ opacity: 0, y: 14 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: Math.min(index * 0.025, 0.45) }}
      onClick={onSelect}
      onPointerMove={handlePointerMove}
      onPointerLeave={resetPointerTilt}
    >
      <CardVisual item={item} styleMode={styleMode} />
      <div className="relative z-10 p-3">
        <h4 className="truncate text-sm font-black text-cv-text">{item.title}</h4>
        <div className="mt-1 flex flex-wrap items-center gap-2 text-[10px] uppercase tracking-[0.08em] text-cv-subtext">
          {item.year && <span>{item.year}</span>}
          <span>{item.media_type}</span>
          {item.resolution && <span className="text-cyan-200">{item.resolution}</span>}
        </div>
        {item.genre && <div className="mt-2 truncate text-[11px] text-cv-subtext/80">{item.genre}</div>}
      </div>
      <div className="cyber-card-actions">
        <button
          type="button"
          onClick={(event) => {
            event.stopPropagation();
            onPlay();
          }}
          className="cyber-button flex-1 text-[10px]"
        >
          <span className="cyber-bracket">[▶]</span> Play
        </button>
        <button
          type="button"
          onClick={(event) => {
            event.stopPropagation();
            onCheckMetadata();
          }}
          disabled={checking}
          className="cyber-button flex-1 text-[10px] disabled:opacity-60"
        >
          {checking ? <RefreshCw size={12} className="animate-spin" /> : <span className="cyber-bracket">[+]</span>}
          Data
        </button>
      </div>
    </motion.div>
  );
}

function CardVisual({ item, styleMode }: { item: MediaItem; styleMode: CardStyle }): JSX.Element {
  if (styleMode === "disc") {
    return (
      <div className="cyber-poster flex aspect-square items-center justify-center p-4">
        <div className="relative aspect-square w-full max-w-[220px] overflow-hidden rounded-full border border-cyan-300/30 bg-gradient-to-br from-cyan-300/20 to-purple-500/20 shadow-[0_0_28px_rgba(0,245,255,0.12)]">
          {resolveMediaImageSrc(item.backdrop_path || item.poster_path) ? (
            <img src={resolveMediaImageSrc(item.backdrop_path || item.poster_path)} alt={item.title} className="h-full w-full object-cover opacity-70" />
          ) : (
            <div className="flex h-full w-full items-center justify-center"><Film size={32} className="text-cv-subtext/30" /></div>
          )}
          <div className="absolute inset-0 bg-black/24" />
          <div className="absolute inset-[18%] rounded-full border border-white/20" />
          <div className="absolute inset-[40%] rounded-full border border-white/20 bg-black/75" />
          <div className="absolute bottom-3 right-3 rounded bg-black/70 px-2 py-1 text-[9px] font-black text-cyan-200">{item.resolution || "HD"}</div>
        </div>
      </div>
    );
  }

  if (styleMode === "banner") {
    return (
      <div className="cyber-poster aspect-[16/9]">
        {resolveMediaImageSrc(item.backdrop_path || item.poster_path) ? (
          <img src={resolveMediaImageSrc(item.backdrop_path || item.poster_path)} alt={item.title} />
        ) : (
          <div className="flex h-full w-full items-center justify-center"><Film size={30} className="text-cv-subtext/30" /></div>
        )}
        <div className="absolute inset-0 bg-gradient-to-t from-black/80 via-black/10 to-transparent" />
        {item.rating && (
          <span className="cyber-chip is-hot absolute right-2 top-2 py-1 text-[10px]"><Star size={10} /> {item.rating}</span>
        )}
      </div>
    );
  }

  return (
    <div className="cyber-poster aspect-[2/3]">
      {resolveMediaImageSrc(item.poster_path || item.backdrop_path) ? (
        <img src={resolveMediaImageSrc(item.poster_path || item.backdrop_path)} alt={item.title} />
      ) : (
        <div className="flex h-full w-full items-center justify-center"><Film size={32} className="text-cv-subtext/30" /></div>
      )}
      <div className="absolute inset-0 bg-gradient-to-t from-black/80 via-transparent to-transparent" />
      <div className="absolute right-2 top-2 flex flex-col gap-1">
        {item.verified && <span className="grid h-6 w-6 place-items-center border border-cyan-200/40 bg-cyan-300/20 text-cyan-100"><CheckCircle size={12} /></span>}
        {item.favorite && <span className="grid h-6 w-6 place-items-center border border-[var(--cyber-amber)]/50 bg-[var(--cyber-amber)]/20 text-[var(--cyber-amber)]"><Heart size={12} /></span>}
      </div>
      {item.resolution && <span className="cyber-chip absolute bottom-2 left-2 py-1 text-[9px]">{item.resolution}</span>}
    </div>
  );
}

const DEMO_ITEMS: MediaItem[] = [
  { title: "Inception", file_path: "", media_type: "movie", year: 2010, rating: 8.8, genre: "Sci-Fi, Thriller", verified: true, watched: true, favorite: true, date_added: "2026-04-01", resolution: "4K", codec: "HEVC", duration: 148, overview: "A thief who steals corporate secrets through dream-sharing technology is given a chance to erase his past." },
  { title: "Breaking Bad", file_path: "", media_type: "tvshow", year: 2008, rating: 9.5, genre: "Drama, Crime", verified: true, watched: false, favorite: false, date_added: "2026-04-02", resolution: "1080p", codec: "H.264", duration: 3000 },
  { title: "Interstellar", file_path: "", media_type: "movie", year: 2014, rating: 8.6, genre: "Sci-Fi, Adventure", verified: false, watched: false, favorite: true, date_added: "2026-04-03", resolution: "4K", codec: "HEVC", duration: 169 },
  { title: "The Dark Knight", file_path: "", media_type: "movie", year: 2008, rating: 9.0, genre: "Action, Crime", verified: true, watched: true, favorite: true, date_added: "2026-04-04", resolution: "1080p", codec: "H.264", duration: 152 },
  { title: "Bohemian Rhapsody", file_path: "", media_type: "music", year: 1975, genre: "Rock", verified: true, watched: false, favorite: true, date_added: "2026-04-05", duration: 6 },
  { title: "Stranger Things", file_path: "", media_type: "tvshow", year: 2016, rating: 8.7, genre: "Horror, Drama", verified: true, watched: true, favorite: false, date_added: "2026-04-06", resolution: "4K", codec: "HEVC", duration: 2100 },
];
