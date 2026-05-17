// CinaVault Premium — Home / Library Tab (Flagship Vidhub-style UI)
import React, { useState, useEffect, useCallback } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { motion } from "framer-motion";
import { useAppStore, MediaItem } from "../../store/appStore";
import { canPlayMediaItem, isLibraryDisplayableMediaItem } from "../../utils/mediaPlaybackSafety";
import ParticleField from "../effects/ParticleField";
import MeteorShower from "../effects/MeteorShower";
import {
  Grid3X3, List, Play, Star, CheckCircle, Clock, Film, Heart, RefreshCw, Sparkles,
  Disc3, RectangleHorizontal, PanelTop, RotateCw
} from "lucide-react";

type Shelf = "recent" | "verified" | "unverified" | "favorites";
type CardStyle = "poster" | "disc" | "banner";

function resolveMediaImageSrc(path?: string | null): string | undefined {
  if (!path) return undefined;
  if (/^(https?:|data:|asset:)/i.test(path)) return path;
  try {
    return convertFileSrc(path);
  } catch {
    return path;
  }
}

export default function HomeTab() {
  const {
    mediaItems, setMediaItems, selectedMedia, setSelectedMedia,
    libraryView, setLibraryView, searchQuery, addStatusMessage
  } = useAppStore();

  const [activeShelf, setActiveShelf] = useState<Shelf>("recent");
  const [filteredItems, setFilteredItems] = useState<MediaItem[]>([]);
  const [typeFilter, setTypeFilter] = useState<string>("all");
  const [loading, setLoading] = useState(false);
  const [iconSize, setIconSize] = useState(148);
  const [cardStyle, setCardStyle] = useState<CardStyle>("poster");
  const [detailFlipped, setDetailFlipped] = useState(false);

  const loadMedia = useCallback(async () => {
    setLoading(true);
    try {
      const items = await invoke<MediaItem[]>("get_media_items");
      setMediaItems(items);
      addStatusMessage(`Library loaded: ${items.length} items (full library)`);
    } catch {
      // Dev mode — use demo data
      setMediaItems(DEMO_ITEMS);
    }
    setLoading(false);
  }, []);

  useEffect(() => { loadMedia(); }, []);

  useEffect(() => {
    let items = mediaItems.filter(isLibraryDisplayableMediaItem);
    if (searchQuery) {
      const q = searchQuery.toLowerCase();
      items = items.filter(m => m.title.toLowerCase().includes(q) || m.genre?.toLowerCase().includes(q));
    }
    if (typeFilter !== "all") items = items.filter(m => m.media_type === typeFilter);
    switch (activeShelf) {
      case "verified": items = items.filter(m => m.verified); break;
      case "unverified": items = items.filter(m => !m.verified); break;
      case "favorites": items = items.filter(m => m.favorite); break;
    }
    setFilteredItems(items);
  }, [mediaItems, searchQuery, typeFilter, activeShelf]);

  useEffect(() => {
    setDetailFlipped(false);
  }, [selectedMedia?.id, selectedMedia?.title]);

  const handlePlay = async (item: MediaItem) => {
    if (!canPlayMediaItem(item)) {
      addStatusMessage(`Play skipped: ${item.title} is not a playable library video/audio item`);
      return;
    }
    try {
      await invoke("play_media", { filePath: item.file_path });
      addStatusMessage(`Playing: ${item.title}`);
    } catch (e) { addStatusMessage(`Play failed: ${e}`); }
  };

  const handleVerify = async (item: MediaItem) => {
    try {
      await invoke("verify_media_item", { id: item.id });
      addStatusMessage(`Verified: ${item.title}`);
      loadMedia();
    } catch {}
  };

  const handleMediaClick = async (item: MediaItem) => {
    setSelectedMedia(item);
  };

  const cardMinWidth = cardStyle === "banner"
    ? Math.max(240, Math.round(iconSize * 1.7))
    : Math.max(112, iconSize);

  return (
    <div className="space-y-5 h-full">
      {/* Spotlight / Hero Section */}
      {selectedMedia && (
        <motion.div
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          className="glass-panel p-0 relative overflow-hidden"
        >
          <MeteorShower meteorCount={50} />
          <div
            className="absolute inset-0 opacity-35"
            style={{
              backgroundImage: selectedMedia.backdrop_path ? `url(${selectedMedia.backdrop_path})` : undefined,
              backgroundSize: "cover",
              backgroundPosition: "center",
            }}
          />
          <div className="absolute inset-0 bg-gradient-to-r from-black/75 via-black/55 to-black/80" />

          <div className="relative z-10 p-5 flex flex-col lg:flex-row gap-6">
            <button
              onClick={() => setDetailFlipped(v => !v)}
              className="detail-card-stage shrink-0 text-left"
              title="Click to rotate card"
            >
              <motion.div
                className="detail-card-inner"
                animate={{ rotateY: detailFlipped ? 180 : 0 }}
                transition={{ duration: 0.65, ease: [0.16, 1, 0.3, 1] }}
              >
                <div className="detail-face detail-face-front border border-cyan-300/20 bg-gradient-to-br from-cv-panel-2/95 to-black/80">
                  {resolveMediaImageSrc(selectedMedia.poster_path) ? (
                    <img src={resolveMediaImageSrc(selectedMedia.poster_path)} alt={selectedMedia.title} className="w-full h-full object-cover opacity-90" />
                  ) : (
                    <div className="w-full h-full flex items-center justify-center">
                      <Film size={46} className="text-cv-accent/60" />
                    </div>
                  )}
                  <div className="absolute inset-x-0 bottom-0 p-3 bg-gradient-to-t from-black/85 via-black/25 to-transparent">
                    <div className="text-[11px] text-cv-subtext mb-1 uppercase tracking-wider">Now Focused</div>
                    <div className="font-semibold truncate">{selectedMedia.title}</div>
                  </div>
                </div>

                <div className="detail-face detail-face-back border border-cyan-300/20 bg-gradient-to-br from-slate-950 to-cyan-950/40 p-4">
                  <div className="text-[11px] uppercase tracking-wider text-cv-accent mb-2">Media Telemetry</div>
                  <div className="space-y-2 text-xs">
                    <DetailMetric label="Type" value={selectedMedia.media_type || "Unknown"} />
                    <DetailMetric label="Resolution" value={selectedMedia.resolution || "N/A"} />
                    <DetailMetric label="Codec" value={selectedMedia.codec || "Auto"} />
                    <DetailMetric label="Runtime" value={selectedMedia.duration ? `${Math.floor(selectedMedia.duration / 60)} min` : "N/A"} />
                    <DetailMetric label="Rating" value={selectedMedia.rating ? `${selectedMedia.rating}` : "N/A"} />
                  </div>
                  <div className="mt-4 text-[11px] text-cv-subtext">Tap to rotate front/back</div>
                </div>
              </motion.div>
            </button>

            <div className="flex-1 min-w-0">
              <h2 className="text-2xl lg:text-3xl font-bold mb-1">{selectedMedia.title}</h2>
              <div className="flex items-center flex-wrap gap-3 text-sm text-cv-subtext mb-3">
                {selectedMedia.year && <span>{selectedMedia.year}</span>}
                {selectedMedia.genre && <span>{selectedMedia.genre}</span>}
                {selectedMedia.resolution && <span className="px-2 py-0.5 rounded bg-cv-accent/20 text-cv-accent text-xs">{selectedMedia.resolution}</span>}
                {selectedMedia.rating && (
                  <span className="flex items-center gap-1"><Star size={12} className="text-cv-gold" />{selectedMedia.rating}</span>
                )}
              </div>
              {selectedMedia.overview && (
                <p className="text-sm text-cv-subtext max-w-3xl leading-relaxed mb-4">{selectedMedia.overview}</p>
              )}

              <div className="flex flex-wrap gap-2">
                <button onClick={() => handlePlay(selectedMedia)} className="cv-btn cv-btn-primary">
                  <Play size={14} /> Play
                </button>
                <button onClick={() => handleVerify(selectedMedia)} className="cv-btn cv-btn-secondary">
                  <CheckCircle size={14} /> Verify
                </button>
                <button onClick={() => setDetailFlipped(v => !v)} className="cv-btn cv-btn-secondary">
                  <RotateCw size={14} /> Rotate Card
                </button>
                <button onClick={() => setSelectedMedia(null)} className="cv-btn cv-btn-secondary">
                  Close
                </button>
              </div>
            </div>
          </div>
        </motion.div>
      )}

      {/* Shelf Tabs + Controls */}
      <div className="glass-panel p-3">
        <div className="flex items-center justify-between gap-3 flex-wrap mb-3">
          <div className="flex gap-1 flex-wrap">
            {([
              { id: "recent" as Shelf, label: "Recently Added", icon: Clock },
              { id: "verified" as Shelf, label: "Verified", icon: CheckCircle },
              { id: "unverified" as Shelf, label: "Needs Metadata", icon: Sparkles },
              { id: "favorites" as Shelf, label: "Favorites", icon: Heart },
            ]).map(s => (
              <button
                key={s.id}
                onClick={() => setActiveShelf(s.id)}
                className={`px-3 py-1.5 rounded-lg text-xs font-semibold flex items-center gap-1.5 transition-all ${
                  activeShelf === s.id
                    ? "bg-cv-accent/15 text-cv-accent border border-cv-accent/20"
                    : "text-cv-subtext hover:text-cv-text hover:bg-white/5"
                }`}
              >
                <s.icon size={12} /> {s.label}
              </button>
            ))}
          </div>

          <div className="flex items-center gap-2 flex-wrap">
            <select
              value={typeFilter}
              onChange={e => setTypeFilter(e.target.value)}
              className="cv-select text-xs py-1.5"
            >
              <option value="all">All Types</option>
              <option value="movie">Movies</option>
              <option value="tvshow">TV Shows</option>
              <option value="music">Music</option>
              <option value="photo">Photos</option>
            </select>

            <div className="flex rounded-lg overflow-hidden border border-white/10">
              <button
                onClick={() => setLibraryView("card")}
                className={`p-1.5 ${libraryView === "card" ? "bg-cv-accent/20 text-cv-accent" : "text-cv-subtext hover:bg-white/5"}`}
              >
                <Grid3X3 size={14} />
              </button>
              <button
                onClick={() => setLibraryView("list")}
                className={`p-1.5 ${libraryView === "list" ? "bg-cv-accent/20 text-cv-accent" : "text-cv-subtext hover:bg-white/5"}`}
              >
                <List size={14} />
              </button>
            </div>

            <button onClick={loadMedia} className="cv-btn cv-btn-secondary text-xs py-1.5">
              <RefreshCw size={12} className={loading ? "animate-spin" : ""} /> Refresh
            </button>
          </div>
        </div>

        {libraryView === "card" && (
          <div className="grid grid-cols-1 lg:grid-cols-[1fr_330px] gap-3">
            <div className="flex items-center gap-2 flex-wrap">
              <button
                onClick={() => setCardStyle("poster")}
                className={`layout-chip ${cardStyle === "poster" ? "active" : ""} px-3 py-1.5 rounded-lg text-xs font-semibold flex items-center gap-1.5`}
              >
                <PanelTop size={12} /> Poster
              </button>
              <button
                onClick={() => setCardStyle("disc")}
                className={`layout-chip ${cardStyle === "disc" ? "active" : ""} px-3 py-1.5 rounded-lg text-xs font-semibold flex items-center gap-1.5`}
              >
                <Disc3 size={12} /> Disc
              </button>
              <button
                onClick={() => setCardStyle("banner")}
                className={`layout-chip ${cardStyle === "banner" ? "active" : ""} px-3 py-1.5 rounded-lg text-xs font-semibold flex items-center gap-1.5`}
              >
                <RectangleHorizontal size={12} /> Banner
              </button>
            </div>
            <div className="flex items-center gap-3">
              <span className="text-[11px] font-semibold uppercase tracking-wider text-cv-subtext">Icon Size</span>
              <input
                type="range"
                min={96}
                max={220}
                step={4}
                value={iconSize}
                onChange={(e) => setIconSize(Number(e.target.value))}
                className="w-full accent-[var(--cv-accent)]"
              />
              <span className="text-xs text-cv-text w-9 text-right">{iconSize}</span>
            </div>
          </div>
        )}
      </div>

      {/* Library Content */}
      {loading ? (
        <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-3">
          {Array.from({ length: 12 }).map((_, i) => (
            <div key={i} className="glass-panel-2 rounded-xl h-56 shimmer" />
          ))}
        </div>
      ) : filteredItems.length === 0 ? (
        <div className="glass-panel p-12 text-center">
          <Film size={48} className="mx-auto text-cv-subtext/30 mb-4" />
          <h3 className="text-lg font-semibold mb-2">No Media Found</h3>
          <p className="text-sm text-cv-subtext">Add media sources and scan to populate your library</p>
        </div>
      ) : libraryView === "card" ? (
        <div
          className="grid gap-3"
          style={{ gridTemplateColumns: `repeat(auto-fill, minmax(${cardMinWidth}px, 1fr))` }}
        >
          {filteredItems.map((item, i) => {
            const className = `media-card glass-panel-2 rounded-xl overflow-hidden group ${cardStyle === "disc" ? "p-3" : ""}`;
            if (filteredItems.length > 500) {
              return (
                <div key={item.id || i} onClick={() => void handleMediaClick(item)} className={className}>
                  <CardVisual item={item} styleMode={cardStyle} />
                  <div className="p-2.5">
                    <h4 className="text-xs font-semibold truncate">{item.title}</h4>
                    <div className="flex items-center gap-2 mt-1 text-[10px] text-cv-subtext">
                      {item.year && <span>{item.year}</span>}
                      <span className="capitalize">{item.media_type}</span>
                    </div>
                  </div>
                </div>
              );
            }
            return (
              <motion.div
                key={item.id || i}
                initial={{ opacity: 0, y: 12 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: Math.min(i * 0.03, 0.5) }}
                onClick={() => void handleMediaClick(item)}
                className={className}
              >
                <CardVisual item={item} styleMode={cardStyle} />
                <div className="p-2.5">
                  <h4 className="text-xs font-semibold truncate">{item.title}</h4>
                  <div className="flex items-center gap-2 mt-1 text-[10px] text-cv-subtext">
                    {item.year && <span>{item.year}</span>}
                    <span className="capitalize">{item.media_type}</span>
                  </div>
                </div>
              </motion.div>
            );
          })}
        </div>
      ) : (
        <div className="glass-panel rounded-xl overflow-hidden">
          <div className="grid grid-cols-[1fr_100px_80px_80px_80px] gap-2 px-4 py-2 border-b border-white/5 text-[10px] font-semibold text-cv-subtext uppercase tracking-wider">
            <span>Title</span><span>Type</span><span>Year</span><span>Rating</span><span>Status</span>
          </div>
          <div className="max-h-[calc(100vh-300px)] overflow-y-auto">
            {filteredItems.map((item, i) => (
              <div
                key={item.id || i}
                onClick={() => void handleMediaClick(item)}
                className="grid grid-cols-[1fr_100px_80px_80px_80px] gap-2 px-4 py-2.5 zebra-row cursor-pointer items-center text-sm"
              >
                <span className="truncate font-medium">{item.title}</span>
                <span className="text-cv-subtext text-xs capitalize">{item.media_type}</span>
                <span className="text-cv-subtext text-xs">{item.year || "—"}</span>
                <span className="text-xs flex items-center gap-1">
                  {item.rating ? <><Star size={10} className="text-cv-gold" />{item.rating}</> : "—"}
                </span>
                <span>{item.verified ? <CheckCircle size={14} className="text-green-500" /> : <Clock size={14} className="text-cv-subtext/40" />}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Stats bar */}
      <div className="flex items-center gap-4 text-[11px] text-cv-subtext">
        <span>{filteredItems.length} items</span>
        <span>{filteredItems.filter(m => m.verified).length} verified</span>
        <span>{filteredItems.filter(m => m.media_type === "movie").length} movies</span>
        <span>{filteredItems.filter(m => m.media_type === "music").length} music</span>
      </div>
    </div>
  );
}

function DetailMetric({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between border-b border-cyan-300/10 pb-1">
      <span className="text-cv-subtext">{label}</span>
      <span className="text-cv-text">{value}</span>
    </div>
  );
}

function CardVisual({ item, styleMode }: { item: MediaItem; styleMode: CardStyle }) {
  if (styleMode === "disc") {
    return (
      <div className="w-full flex items-center justify-center py-2">
        <div className="relative w-full aspect-square max-w-[220px] rounded-full border border-cyan-300/20 bg-gradient-to-br from-cv-accent/20 to-cv-neon-3/10 overflow-hidden">
          {resolveMediaImageSrc(item.backdrop_path) && <img src={resolveMediaImageSrc(item.backdrop_path)} alt={item.title} className="w-full h-full object-cover opacity-65" />}
          <div className="absolute inset-0 bg-black/30" />
          <div className="absolute inset-[18%] rounded-full border border-white/20" />
          <div className="absolute inset-[40%] rounded-full bg-black/65 border border-white/20" />
          <div className="absolute bottom-2 right-2 text-[9px] px-1.5 py-0.5 rounded bg-black/65 text-cv-accent">
            {item.resolution || "HD"}
          </div>
        </div>
      </div>
    );
  }

  if (styleMode === "banner") {
    return (
      <div className="aspect-[16/9] relative bg-gradient-to-br from-cv-accent/10 to-cv-neon-3/10 flex items-center justify-center">
        {resolveMediaImageSrc(item.backdrop_path) ? (
          <img src={resolveMediaImageSrc(item.backdrop_path)} alt={item.title} className="w-full h-full object-cover" />
        ) : (
          <Film size={30} className="text-cv-subtext/20" />
        )}
        <div className="absolute inset-0 bg-gradient-to-t from-black/75 to-transparent" />
        {item.rating && (
          <span className="absolute top-2 right-2 text-[10px] px-2 py-0.5 rounded bg-black/65 text-cv-gold flex items-center gap-1">
            <Star size={10} /> {item.rating}
          </span>
        )}
      </div>
    );
  }

  return (
    <div className="aspect-[2/3] relative bg-gradient-to-br from-cv-accent/10 to-cv-neon-3/10 flex items-center justify-center">
      {resolveMediaImageSrc(item.poster_path) ? (
        <img src={resolveMediaImageSrc(item.poster_path)} alt={item.title} className="w-full h-full object-cover" />
      ) : (
        <Film size={32} className="text-cv-subtext/20" />
      )}
      <div className="absolute inset-0 bg-black/60 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center">
        <span className="w-12 h-12 rounded-full bg-cv-accent flex items-center justify-center shadow-lg">
          <Play size={20} fill="white" color="white" />
        </span>
      </div>
      <div className="absolute top-2 right-2 flex flex-col gap-1">
        {item.verified && (
          <span className="w-5 h-5 rounded-full bg-green-500/80 flex items-center justify-center">
            <CheckCircle size={10} color="white" />
          </span>
        )}
        {item.favorite && (
          <span className="w-5 h-5 rounded-full bg-cv-danger/80 flex items-center justify-center">
            <Heart size={10} color="white" fill="white" />
          </span>
        )}
      </div>
      {item.resolution && (
        <span className="absolute bottom-2 left-2 text-[9px] font-bold px-1.5 py-0.5 rounded bg-black/60 text-cv-accent">
          {item.resolution}
        </span>
      )}
    </div>
  );
}

// Demo data for development mode
const DEMO_ITEMS: MediaItem[] = [
  { title: "Inception", file_path: "", media_type: "movie", year: 2010, rating: 8.8, genre: "Sci-Fi, Thriller", verified: true, watched: true, favorite: true, date_added: "2026-04-01", resolution: "4K", overview: "A thief who steals corporate secrets through dream-sharing technology." },
  { title: "Breaking Bad", file_path: "", media_type: "tvshow", year: 2008, rating: 9.5, genre: "Drama, Crime", verified: true, watched: false, favorite: false, date_added: "2026-04-02", resolution: "1080p" },
  { title: "Interstellar", file_path: "", media_type: "movie", year: 2014, rating: 8.6, genre: "Sci-Fi, Adventure", verified: false, watched: false, favorite: true, date_added: "2026-04-03", resolution: "4K" },
  { title: "The Dark Knight", file_path: "", media_type: "movie", year: 2008, rating: 9.0, genre: "Action, Crime", verified: true, watched: true, favorite: true, date_added: "2026-04-04", resolution: "1080p" },
  { title: "Bohemian Rhapsody", file_path: "", media_type: "music", year: 1975, genre: "Rock", verified: true, watched: false, favorite: true, date_added: "2026-04-05" },
  { title: "Stranger Things", file_path: "", media_type: "tvshow", year: 2016, rating: 8.7, genre: "Horror, Drama", verified: true, watched: true, favorite: false, date_added: "2026-04-06", resolution: "4K" },
];