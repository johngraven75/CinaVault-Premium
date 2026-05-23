// CinaVault Premium — Home / Library Tab
import React, { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence } from "framer-motion";
import { useAppStore, MediaItem } from "../../store/appStore";
import ParticleField from "../effects/ParticleField";
import {
    Grid3X3, List, Play, Star, CheckCircle, Clock, Film, Music,
    Heart, RefreshCw, Sparkles, Layers, ImageIcon, Bot
} from "lucide-react";

type Shelf = "recent" | "verified" | "unverified" | "favorites" | "alphabetical";
type LetterSection = {
  letter: string;
  items: MediaItem[];
};

export default function HomeTab() {
  const {
    mediaItems, setMediaItems, selectedMedia, setSelectedMedia,
    libraryView, setLibraryView, searchQuery, addStatusMessage, settings, setSetting
  } = useAppStore();

  const [activeShelf, setActiveShelf] = useState<Shelf>("recent");
  const [filteredItems, setFilteredItems] = useState<MediaItem[]>([]);
  const [typeFilter, setTypeFilter] = useState<string>("all");
  const [loading, setLoading] = useState(false);
  const [homeMode, setHomeMode] = useState<"library" | "tasks">("library");
  const [chapterImages, setChapterImages] = useState(true);
  const [metadataGather, setMetadataGather] = useState(true);
  const [metadataAgents, setMetadataAgents] = useState<string[]>(["tmdb", "omdb"]);

  const loadMedia = useCallback(async () => {
    setLoading(true);
    try {
      const items = await invoke<MediaItem[]>("get_media_items", { limit: 200 });
      setMediaItems(items);
      addStatusMessage(`Library loaded: ${items.length} items`);
    } catch {
      // Dev mode — use demo data
      setMediaItems(DEMO_ITEMS);
    }
    setLoading(false);
  }, []);

  useEffect(() => { loadMedia(); }, []);

  useEffect(() => {
    setChapterImages(settings.library_task_chapter_images !== "false");
    setMetadataGather(settings.library_task_metadata_gather !== "false");
    const agents = settings.library_task_metadata_agents;
    if (agents) {
      const parsed = agents.split(",").map((a) => a.trim()).filter(Boolean);
      if (parsed.length) setMetadataAgents(parsed);
    }
  }, [settings.library_task_chapter_images, settings.library_task_metadata_gather, settings.library_task_metadata_agents]);

   useEffect(() => {
     let items = [...mediaItems];
     if (searchQuery) {
       const q = searchQuery.toLowerCase();
       items = items.filter(m => m.title.toLowerCase().includes(q) || m.genre?.toLowerCase().includes(q));
     }
     if (typeFilter !== "all") items = items.filter(m => m.media_type === typeFilter);
     switch (activeShelf) {
       case "verified": items = items.filter(m => m.verified); break;
       case "unverified": items = items.filter(m => !m.verified); break;
       case "favorites": items = items.filter(m => m.favorite); break;
       case "alphabetical": {
         // Group items by first letter of title
         const grouped: Record<string, MediaItem[]> = {};
         items.forEach(item => {
           const firstChar = item.title.charAt(0).toUpperCase();
           if (!grouped[firstChar]) {
             grouped[firstChar] = [];
           }
           grouped[firstChar].push(item);
         });
         
         // Convert to array of sections sorted by letter
         const letterSections: LetterSection[] = Object.keys(grouped)
           .sort()
           .map(letter => ({
             letter,
             items: grouped[letter]
           }));
         
         // Create a new array with section headers
         const newItems: MediaItem[] = [];
         letterSections.forEach(section => {
           // Add header item
           newItems.push({
             title: `--- ${section.letter} ---`,
             isHeader: true,
             file_path: "",
             media_type: "header",
             year: 0,
             rating: 0,
             overview: "",
             poster_path: "",
             backdrop_path: "",
             genre: "",
             duration: 0,
             file_size: 0,
             resolution: "",
             codec: "",
             verified: false,
             watched: false,
             favorite: false,
             date_added: "",
             last_played: undefined,
             tmdb_id: undefined,
             imdb_id: undefined,
             source_id: undefined
           });
           
           // Add actual items
           section.items.forEach(item => {
             newItems.push({ ...item, isHeader: false });
           });
         });
         
         items = newItems;
         break;
       }
     }
     setFilteredItems(items);
   }, [mediaItems, searchQuery, typeFilter, activeShelf]);

  const handlePlay = async (item: MediaItem) => {
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

  const persistTaskSetting = async (key: string, value: string) => {
    setSetting(key, value);
    try {
      await invoke("set_setting", { key, value });
    } catch {
      // Ignore persistence in non-tauri dev mode
    }
  };

  const toggleMetadataAgent = async (agent: string) => {
    const next = metadataAgents.includes(agent)
      ? metadataAgents.filter((a) => a !== agent)
      : [...metadataAgents, agent];
    setMetadataAgents(next);
    await persistTaskSetting("library_task_metadata_agents", next.join(","));
  };

  return (
    <div className="space-y-5 h-full">
      {/* Spotlight / Hero Section */}
      {selectedMedia && (
        <motion.div
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          className="glass-panel p-5 relative overflow-hidden"
        >
          <ParticleField particleCount={30} />
          <div className="relative z-10 flex gap-5">
            {/* Poster placeholder */}
            <div className="w-32 h-48 rounded-xl bg-gradient-to-br from-cv-accent/20 to-cv-neon-3/20 flex items-center justify-center shrink-0 border border-white/5">
              <Film size={36} className="text-cv-accent/40" />
            </div>
            <div className="flex-1 min-w-0">
              <h2 className="text-2xl font-bold mb-1">{selectedMedia.title}</h2>
              <div className="flex items-center gap-3 text-sm text-cv-subtext mb-3">
                {selectedMedia.year && <span>{selectedMedia.year}</span>}
                {selectedMedia.genre && <span>{selectedMedia.genre}</span>}
                {selectedMedia.resolution && <span className="px-2 py-0.5 rounded bg-cv-accent/20 text-cv-accent text-xs">{selectedMedia.resolution}</span>}
                {selectedMedia.rating && (
                  <span className="flex items-center gap-1"><Star size={12} className="text-cv-gold" />{selectedMedia.rating}</span>
                )}
              </div>
              {selectedMedia.overview && (
                <p className="text-sm text-cv-subtext line-clamp-3 mb-4">{selectedMedia.overview}</p>
              )}
              <div className="flex gap-2">
                <button onClick={() => handlePlay(selectedMedia)} className="cv-btn cv-btn-primary">
                  <Play size={14} /> Play
                </button>
                <button onClick={() => handleVerify(selectedMedia)} className="cv-btn cv-btn-secondary">
                  <CheckCircle size={14} /> Verify
                </button>
                <button onClick={() => setSelectedMedia(null)} className="cv-btn cv-btn-secondary">
                  Close
                </button>
              </div>
            </div>
          </div>
        </motion.div>
      )}

       {/* Carousel Shelf Tabs */}
       <div className="flex items-center justify-between">
         <div className="flex gap-1">
           {([
             { id: "recent" as Shelf, label: "Recently Added", icon: Clock },
             { id: "verified" as Shelf, label: "Verified", icon: CheckCircle },
             { id: "unverified" as Shelf, label: "Needs Metadata", icon: Sparkles },
             { id: "favorites" as Shelf, label: "Favorites", icon: Heart },
             { id: "alphabetical" as Shelf, label: "Alphabetical", icon: "A-Z" },
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
                {s.id === "alphabetical" ? (
                  <span className="flex items-center gap-1 text-cv-accent">
                    A<span className="mx-1">-</span>Z
                  </span>
                ) : (
                  <s.icon size={12} />
                )}
               {s.label}
             </button>
           ))}
         </div>

        <div className="flex items-center gap-2">
          {/* Type filter */}
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

          {/* View toggle */}
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

      <div className="flex gap-2">
        <button onClick={() => setHomeMode("library")} className={`cv-btn ${homeMode === "library" ? "cv-btn-primary" : "cv-btn-secondary"} text-xs`}>
          <Layers size={12} /> Library View
        </button>
        <button onClick={() => setHomeMode("tasks")} className={`cv-btn ${homeMode === "tasks" ? "cv-btn-primary" : "cv-btn-secondary"} text-xs`}>
          <Bot size={12} /> Library Tasks
        </button>
      </div>

      {/* Library Content */}
      {homeMode === "tasks" ? (
        <div className="glass-panel p-5 space-y-4">
          <h3 className="text-sm font-bold flex items-center gap-2">
            <Bot size={16} className="text-cv-accent" /> Library Task Sub-Tab
          </h3>
          <div className="glass-panel-2 p-4 rounded-lg space-y-3">
            <div className="flex items-center justify-between">
              <div>
                <div className="text-sm font-semibold flex items-center gap-2"><ImageIcon size={14} className="text-cv-accent" /> Chapter Image Selection</div>
                <div className="text-[10px] text-cv-subtext">Enable chapter image generation and selection during library tasks.</div>
              </div>
              <div className={`cv-toggle ${chapterImages ? "active" : ""}`} onClick={async () => {
                const next = !chapterImages;
                setChapterImages(next);
                await persistTaskSetting("library_task_chapter_images", String(next));
              }} />
            </div>
            <div className="flex items-center justify-between">
              <div>
                <div className="text-sm font-semibold">Metadata Gathering Options</div>
                <div className="text-[10px] text-cv-subtext">Run metadata gathering during library refresh and scan tasks.</div>
              </div>
              <div className={`cv-toggle ${metadataGather ? "active" : ""}`} onClick={async () => {
                const next = !metadataGather;
                setMetadataGather(next);
                await persistTaskSetting("library_task_metadata_gather", String(next));
              }} />
            </div>
          </div>
          <div className="glass-panel-2 p-4 rounded-lg">
            <div className="text-sm font-semibold mb-2">Metadata Agent Options</div>
            <div className="grid grid-cols-2 md:grid-cols-4 gap-2">
              {["tmdb", "omdb", "tvmaze", "musicbrainz"].map((agent) => {
                const enabled = metadataAgents.includes(agent);
                return (
                  <button key={agent} onClick={() => toggleMetadataAgent(agent)} className={`cv-btn ${enabled ? "cv-btn-primary" : "cv-btn-secondary"} text-xs justify-center`}>
                    {enabled ? <CheckCircle size={12} /> : <Clock size={12} />} {agent.toUpperCase()}
                  </button>
                );
              })}
            </div>
          </div>
        </div>
      ) : loading ? (
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
         <div className="space-y-4">
           {filteredItems.map((item, index) => {
             // Check if this is a header item for alphabetical view
             if (activeShelf === "alphabetical" && item.isHeader) {
               return (
                 <div key={`header-${index}`} className="text-xs font-bold text-cv-accent mb-2">
                   {item.title}
                 </div>
               );
             }
             
             return (
               <motion.div
                 key={item.id || index}
                 initial={{ opacity: 0, y: 12 }}
                 animate={{ opacity: 1, y: 0 }}
                 transition={{ delay: Math.min(index * 0.03, 0.5) }}
                 onClick={() => setSelectedMedia(item)}
                 className="media-card glass-panel-2 rounded-xl overflow-hidden group"
               >
                 {/* Poster area */}
                 <div className="aspect-[2/3] relative bg-gradient-to-br from-cv-accent/10 to-cv-neon-3/10 flex items-center justify-center">
                   {item.poster_path ? (
                     <img src={item.poster_path} alt={item.title} className="w-full h-full object-cover" />
                   ) : (
                     <Film size={32} className="text-cv-subtext/20" />
                   )}
                   {/* Hover overlay */}
                   <div className="absolute inset-0 bg-black/60 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center">
                     <button
                       onClick={(e) => { e.stopPropagation(); handlePlay(item); }}
                       className="w-12 h-12 rounded-full bg-cv-accent flex items-center justify-center shadow-lg"
                     >
                       <Play size={20} fill="white" color="white" />
                     </button>
                   </div>
                   {/* Badges */}
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
                 {/* Info */}
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
         /* List View */
         <div className="glass-panel rounded-xl overflow-hidden">
           <div className="grid grid-cols-[1fr_100px_80px_80px_80px] gap-2 px-4 py-2 border-b border-white/5 text-[10px] font-semibold text-cv-subtext uppercase tracking-wider">
             <span>Title</span><span>Type</span><span>Year</span><span>Rating</span><span>Status</span>
           </div>
           <div className="max-h-[calc(100vh-300px)] overflow-y-auto">
             {filteredItems.map((item, index) => {
               // Check if this is a header item for alphabetical view
               if (activeShelf === "alphabetical" && item.isHeader) {
                 return (
                   <div key={`header-${index}`} className="text-xs font-bold text-cv-accent mb-2">
                     {item.title}
                   </div>
                 );
               }
               
               return (
                 <div
                   key={item.id || index}
                   onClick={() => setSelectedMedia(item)}
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
               );
             })}
           </div>
         </div>
       )}

      {homeMode === "library" && (
        <div className="flex items-center gap-4 text-[11px] text-cv-subtext">
          <span>{filteredItems.length} items</span>
          <span>{filteredItems.filter(m => m.verified).length} verified</span>
          <span>{filteredItems.filter(m => m.media_type === "movie").length} movies</span>
          <span>{filteredItems.filter(m => m.media_type === "music").length} music</span>
        </div>
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
