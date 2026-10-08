// Genre Radio: pick one of the library's genres and play a shuffled queue of
// its titles through the shared playback entry point.
import { useEffect, useState } from "react";
import type { JSX } from "react";
import { Radio, RefreshCw } from "lucide-react";

import { useFeature } from "../../features/featureFlags";
import {
  fetchGenreQueue,
  fetchGenres,
  GENRE_RADIO_DEFAULTS,
  genreRadioQueue,
  stationSize,
  type GenreCount,
  type GenreRadioConfig,
} from "../../services/discovery";
import { playMedia } from "../../services/playback";
import { useAppStore } from "../../store/appStore";
import { canPlayMediaItem } from "../../utils/mediaPlaybackSafety";
import type { DiscoverySkin } from "./DiscoveryCard";

export default function GenreRadio({ skin, reloadKey }: { skin: DiscoverySkin; reloadKey: number }): JSX.Element | null {
  const { enabled, config } = useFeature<GenreRadioConfig>("genre_radio", GENRE_RADIO_DEFAULTS);
  const addStatusMessage = useAppStore((state) => state.addStatusMessage);
  const [genres, setGenres] = useState<GenreCount[]>([]);
  const [genre, setGenre] = useState("");
  const [starting, setStarting] = useState(false);

  useEffect(() => {
    if (!enabled) return;
    let active = true;
    fetchGenres()
      .then((list) => {
        if (!active) return;
        setGenres(list);
        setGenre((current) => (list.some((entry) => entry.name === current) ? current : list[0]?.name ?? ""));
      })
      .catch((error) => addStatusMessage(`Genre Radio could not list genres: ${String(error)}`));
    return () => { active = false; };
  }, [enabled, reloadKey, addStatusMessage]);

  if (!enabled) return null;

  const start = async () => {
    if (!genre) return;
    setStarting(true);
    try {
      const size = stationSize(config.queueSize);
      const queue = genreRadioQueue(await fetchGenreQueue(genre, size), canPlayMediaItem);
      if (!queue.length) {
        addStatusMessage(`Genre Radio: no playable ${genre} titles`);
        return;
      }
      await playMedia(queue, 0, { source: "genre_radio", shuffle: true });
      addStatusMessage(`Genre Radio: ${genre}, ${queue.length} titles shuffled. Now playing ${queue[0].title}`);
    } catch (error) {
      addStatusMessage(`Genre Radio failed: ${String(error)}`);
    } finally {
      setStarting(false);
    }
  };

  return (
    <div className={`cv-genre-radio is-${skin}`} role="group" aria-label="Genre Radio">
      <span className="cv-genre-radio__label"><Radio size={14} /> Genre Radio</span>
      {genres.length ? (
        <>
          <select value={genre} onChange={(event) => setGenre(event.target.value)} aria-label="Genre station">
            {genres.map((entry) => (
              <option key={entry.name} value={entry.name}>
                {entry.name} ({entry.count})
              </option>
            ))}
          </select>
          <button type="button" onClick={() => void start()} disabled={starting || !genre} className="cv-genre-radio__play">
            {starting ? <RefreshCw size={13} className="animate-spin" /> : <Radio size={13} />} {starting ? "Tuning…" : "Play station"}
          </button>
        </>
      ) : (
        <span className="cv-genre-radio__empty">No genres yet. Fetch metadata to fill in genres.</span>
      )}
    </div>
  );
}
