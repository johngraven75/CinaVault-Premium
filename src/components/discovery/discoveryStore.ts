// Shared watchlist state for the home screen: card buttons, detail panels and
// the Watchlist shelf all read it, so a toggle anywhere updates everywhere.
import { create } from "zustand";

import type { MediaItem } from "../../store/appStore";
import {
  fetchWatchlist,
  isOnWatchlist,
  itemMediaIds,
  toggleWatchlistEntry,
} from "../../services/discovery";

interface DiscoveryStore {
  watchlist: MediaItem[];
  watchlistIds: Set<number>;
  /** Bumped after a change that other shelves (recommendations) depend on. */
  revision: number;
  loadWatchlist: () => Promise<void>;
  /** Adds or removes the title; returns whether it is now on the watchlist. */
  toggle: (item: MediaItem) => Promise<boolean>;
}

export const useDiscoveryStore = create<DiscoveryStore>((set, get) => ({
  watchlist: [],
  watchlistIds: new Set(),
  revision: 0,
  loadWatchlist: async () => {
    const watchlist = await fetchWatchlist();
    set({
      watchlist,
      watchlistIds: new Set(watchlist.map((item) => item.id).filter((id): id is number => typeof id === "number")),
    });
  },
  toggle: async (item) => {
    const ids = itemMediaIds(item);
    if (!ids.length) throw new Error(`${item.title} has no library id yet`);
    const { watchlistIds } = get();
    if (isOnWatchlist(item, watchlistIds)) {
      // Remove every copy that is on the list (it may have been saved from another copy).
      for (const id of ids.filter((id) => watchlistIds.has(id))) await toggleWatchlistEntry(id);
    } else {
      await toggleWatchlistEntry(ids[0]);
    }
    await get().loadWatchlist();
    set((state) => ({ revision: state.revision + 1 }));
    return isOnWatchlist(item, get().watchlistIds);
  },
}));
