// Lookup picker dialog (Issue 5, Tier 2): search a metadata provider for a
// library item, rank the hits, and let the user apply one candidate.
//
// Wiring, all verified against main e210d969:
// - Provider list: `get_metadata_providers` (backend shape: name/key/base_url/
//   requires_key/category). The list is filtered with the repo's own edition
//   helpers — `filterStoreSafeProviders` (no adult providers in the store-safe
//   build) then `applyLibraryProviderPolicy` (regular providers are "not in
//   use" in the full edition) — so the select only offers providers the
//   backend's `search_metadata` will actually accept. `get_provider_status`
//   marks which providers have a saved key (`configured` map).
// - Search: `search_metadata` with exactly the arguments the existing
//   `searchAndIdentify` helper uses ({ provider, query, mediaType, apiKey }).
//   The raw response is normalized with `normalizeSearchResults` and ranked
//   with `scoreCandidates` from posterIdentification — the same title/year
//   normalisation the AI poster autopilot uses. No vision call is made here:
//   without a reference image the image signal is absent and title/year carry
//   the ranking (scoreCandidates renormalizes the remaining weights).
// - Apply: `metadata_apply_match_item` (new backend command). Its nested
//   `candidate` fields use snake_case, matching the Rust struct exactly — the
//   same convention as MediaItem (no serde rename on either struct). Fields
//   sent as null are preserved server-side (COALESCE), never cleared: a search
//   candidate carries title/overview/poster/year/tmdb/imdb, but no
//   backdrop/genre/rating, so those stay as they were.
// - Refresh: on success the dialog hands the returned `updated_item` to
//   `onApplied` (HomeTab feeds it to `applyUpdatedMediaItem`, the same path
//   Check metadata uses) and then closes. Errors stay in the dialog as an
//   inline status message; nothing is written on a failed apply.

import { useCallback, useEffect, useRef, useState } from "react";
import type { JSX } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw, Search, X } from "lucide-react";
import type { MediaItem, MetadataProvider } from "../store/appStore";
import { filterStoreSafeProviders, IS_STORE_SAFE } from "../config/edition";
import { applyLibraryProviderPolicy } from "../services/libraryMetadataPolicy";
import {
  normalizeSearchResults,
  scoreCandidates,
  type CandidateScore,
} from "../services/posterIdentification";

/** Backend `get_metadata_providers` row (serde field names, no rename). */
interface BackendProvider {
  name: string;
  key: string;
  base_url: string;
  requires_key: boolean;
  category: string;
}

/** Backend `get_provider_status` shape: configured keys map to true. */
interface ProviderStatus {
  total_providers?: number;
  configured?: Record<string, boolean>;
}

interface ApplyMatchResult {
  updated_item?: Partial<MediaItem> & { id?: number };
}

/**
 * Mirrors the backend's normalize_provider_key aliases (metadata_ext.rs) so a
 * saved key is recognised even when the provider list spells the key with an
 * alias (themoviedb -> tmdb, theporndb -> tpdb, open_movie_db -> omdb).
 */
function normalizeConfiguredKey(key: string): string {
  switch (key.trim().toLowerCase()) {
    case "themoviedb":
    case "themoviedb_images":
    case "tmdb_images":
      return "tmdb";
    case "theporndb":
      return "tpdb";
    case "open_movie_db":
    case "openmoviedb":
      return "omdb";
    default:
      return key.trim().toLowerCase();
  }
}

export interface MetadataLookupDialogProps {
  item: MediaItem;
  onClose: () => void;
  onApplied: (updated: Partial<MediaItem> & { id?: number }) => void;
}

export default function MetadataLookupDialog({
  item,
  onClose,
  onApplied,
}: MetadataLookupDialogProps): JSX.Element {
  const [providers, setProviders] = useState<
    { id: string; name: string; category: string; requiresKey: boolean; configured: boolean }[]
  >([]);
  const [providersLoading, setProvidersLoading] = useState(true);
  const [providerKey, setProviderKey] = useState("");
  const [query, setQuery] = useState(item.title);
  const [candidates, setCandidates] = useState<CandidateScore[]>([]);
  const [searching, setSearching] = useState(false);
  const [searched, setSearched] = useState(false);
  const [applyingId, setApplyingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const autoSearchedProviderRef = useRef<string | null>(null);

  useEffect(() => {
    let active = true;
    void (async () => {
      try {
        const [backendProviders, status] = await Promise.all([
          invoke<BackendProvider[]>("get_metadata_providers"),
          invoke<ProviderStatus>("get_provider_status"),
        ]);
        if (!active) return;
        const configured = status.configured ?? {};
        const configuredKeys = new Set(
          Object.entries(configured)
            .filter(([, value]) => value)
            .map(([key]) => normalizeConfiguredKey(key)),
        );
        // Map to the store's MetadataProvider shape FIRST, because
        // filterStoreSafeProviders returns MetadataProvider[] and would erase
        // the dialog's requiresKey/configured fields from the type if they
        // were attached before filtering. They are re-attached below from a
        // lookup, so the policy helpers still see exactly their own shape.
        const storeProviders: MetadataProvider[] = backendProviders.map((provider) => ({
          id: provider.key,
          name: provider.name,
          category: provider.category,
          enabled: true,
        }));
        const backendById = new Map(backendProviders.map((provider) => [provider.key, provider]));
        const mapped = filterStoreSafeProviders(storeProviders).map((provider) => ({
          ...provider,
          requiresKey: backendById.get(provider.id)?.requires_key ?? false,
          configured: configuredKeys.has(normalizeConfiguredKey(provider.id)),
        }));
        const usable = applyLibraryProviderPolicy(mapped, IS_STORE_SAFE).filter(
          (provider) => provider.inUse !== false,
        );
        setProviders(usable);
        if (usable.length > 0) setProviderKey(usable[0].id);
        else setError("No metadata providers are available for Lookup in this edition.");
      } catch (caught) {
        if (active) setError(`Provider list unavailable: ${String(caught)}`);
      } finally {
        if (active) setProvidersLoading(false);
      }
    })();
    return () => {
      active = false;
    };
  }, []);

  const runSearch = useCallback(
    async (searchQuery: string, searchProvider: string) => {
      if (!searchProvider || !searchQuery.trim()) return;
      setSearching(true);
      setError(null);
      try {
        const raw = await invoke<unknown>("search_metadata", {
          provider: searchProvider,
          query: searchQuery.trim(),
          mediaType: item.media_type,
          apiKey: null,
        });
        const normalized = normalizeSearchResults(searchProvider, raw);
        const ranked = scoreCandidates(
          { title: item.title, media_type: item.media_type, year: item.year, id: item.id },
          normalized,
        );
        setCandidates(ranked);
        setSearched(true);
      } catch (caught) {
        setCandidates([]);
        setSearched(true);
        setError(`Lookup failed: ${String(caught)}`);
      } finally {
        setSearching(false);
      }
    },
    [item],
  );

  // First search runs automatically once the provider list lands, using the
  // card title as the query — the user only types when that misses.
  useEffect(() => {
    if (providersLoading || !providerKey) return;
    if (autoSearchedProviderRef.current === providerKey) return;
    autoSearchedProviderRef.current = providerKey;
    void runSearch(query, providerKey);
    // Only the provider choice retriggers the automatic search; typing a new
    // query waits for the Search button. `query` is intentionally not a dep.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [providersLoading, providerKey, runSearch]);

  const applyCandidate = async (scored: CandidateScore) => {
    if (!item.id || applyingId) return;
    const candidate = scored.candidate;
    setApplyingId(candidate.id);
    setError(null);
    try {
      const result = await invoke<ApplyMatchResult>("metadata_apply_match_item", {
        id: item.id,
        candidate: {
          title: candidate.title,
          overview: candidate.overview ?? null,
          poster_path: candidate.posterUrl ?? null,
          backdrop_path: null,
          year: candidate.year ?? null,
          rating: null,
          genre: null,
          tmdb_id: candidate.tmdbId ?? null,
          imdb_id: candidate.imdbId ?? null,
          media_type: null,
        },
      });
      onApplied(
        result.updated_item ?? {
          id: item.id,
          title: candidate.title,
          overview: candidate.overview ?? undefined,
          poster_path: candidate.posterUrl ?? undefined,
          year: candidate.year ?? undefined,
          tmdb_id: candidate.tmdbId ?? undefined,
          imdb_id: candidate.imdbId ?? undefined,
        },
      );
      onClose();
    } catch (caught) {
      setError(`Apply failed: ${String(caught)}`);
    } finally {
      setApplyingId(null);
    }
  };

  const selectedProvider = providers.find((provider) => provider.id === providerKey);

  return (
    <div
      className="fixed inset-0 z-[120] flex items-center justify-center bg-black/70 p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="cv-lookup-title"
      onKeyDown={(event) => {
        if (event.key === "Escape") onClose();
      }}
    >
      <button
        type="button"
        tabIndex={-1}
        aria-label={`Close lookup for ${item.title}`}
        className="absolute inset-0 h-full w-full cursor-default bg-transparent"
        onClick={onClose}
      />
      <div className="cyber-panel relative w-full max-w-lg rounded-[18px] p-5">
        <div className="flex items-start justify-between gap-3">
          <div>
            <h3 id="cv-lookup-title" className="text-lg font-black uppercase tracking-[0.12em] text-cv-text">
              Lookup metadata
            </h3>
            <p className="mt-1 text-sm text-cv-subtext">
              Pick the right match for “{item.title}”. Applying overwrites only the fields the
              candidate carries; everything else stays as it is.
            </p>
          </div>
          <button type="button" onClick={onClose} className="cyber-button h-10 w-10 px-0" title="Close lookup">
            <X size={15} />
          </button>
        </div>

        <div className="mt-4 grid gap-2">
          <label className="text-xs font-bold uppercase tracking-[0.12em] text-cv-subtext" htmlFor="cv-lookup-provider">
            Provider
          </label>
          <select
            id="cv-lookup-provider"
            value={providerKey}
            onChange={(event) => setProviderKey(event.target.value)}
            disabled={providersLoading || providers.length === 0}
            className="cyber-select"
          >
            {providers.map((provider) => (
              <option key={provider.id} value={provider.id}>
                {provider.name} ({provider.category})
                {provider.requiresKey && !provider.configured ? " — key required" : ""}
              </option>
            ))}
          </select>
          {selectedProvider?.requiresKey && !selectedProvider.configured && (
            <p className="text-xs text-amber-200">
              This provider needs an API key. Save one in Settings first, or the search will fail.
            </p>
          )}
          <label className="text-xs font-bold uppercase tracking-[0.12em] text-cv-subtext" htmlFor="cv-lookup-query">
            Search
          </label>
          <div className="flex gap-2">
            <input
              id="cv-lookup-query"
              type="text"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  void runSearch(query, providerKey);
                }
              }}
              placeholder={item.title}
              className="cyber-select min-w-0 flex-1"
              aria-label="Lookup query"
            />
            <button
              type="button"
              onClick={() => void runSearch(query, providerKey)}
              disabled={searching || !providerKey || !query.trim()}
              className="cyber-button is-amber disabled:opacity-60"
            >
              {searching ? <RefreshCw size={14} className="animate-spin" /> : <Search size={14} />} Search
            </button>
          </div>
        </div>

        {error && (
          <p role="status" className="mt-4 text-sm text-amber-200">
            {error}
          </p>
        )}

        <div className="mt-4 max-h-80 overflow-y-auto pr-1">
          {providersLoading || searching ? (
            <p role="status" className="text-sm text-cv-subtext">
              {providersLoading ? "Loading providers…" : "Searching…"}
            </p>
          ) : searched && candidates.length === 0 && !error ? (
            <p role="status" className="text-sm text-cv-subtext">
              No candidates found. Try a shorter query or another provider.
            </p>
          ) : (
            <ul className="grid gap-2">
              {candidates.map((scored) => {
                const candidate = scored.candidate;
                return (
                  <li
                    key={candidate.id}
                    className="flex items-center gap-3 rounded border border-cyan-300/15 bg-black/30 p-2"
                  >
                    {candidate.posterUrl ? (
                      <img
                        src={candidate.posterUrl}
                        alt=""
                        loading="lazy"
                        className="h-16 w-11 shrink-0 rounded object-cover"
                      />
                    ) : (
                      <span className="grid h-16 w-11 shrink-0 place-items-center rounded bg-black/40 text-cv-subtext">
                        <Search size={14} />
                      </span>
                    )}
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-sm font-bold text-cv-text">
                        {candidate.title}
                      </span>
                      <span className="block text-xs text-cv-subtext">
                        {candidate.provider}
                        {candidate.year ? ` · ${candidate.year}` : ""}
                        {candidate.mediaType ? ` · ${candidate.mediaType}` : ""} ·{" "}
                        {Math.round(scored.score * 100)}% match
                      </span>
                      {candidate.overview && (
                        <span className="mt-1 block truncate text-xs text-cv-subtext">
                          {candidate.overview}
                        </span>
                      )}
                    </span>
                    <button
                      type="button"
                      onClick={() => void applyCandidate(scored)}
                      disabled={applyingId !== null || !item.id}
                      className="cyber-button is-amber shrink-0 disabled:opacity-60"
                      aria-label={`Apply ${candidate.title} to ${item.title}`}
                    >
                      {applyingId === candidate.id ? "Applying…" : "Apply"}
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </div>

        {!item.id && (
          <p role="status" className="mt-4 text-sm text-amber-200">
            This item has no vault id yet, so a match cannot be saved for it.
          </p>
        )}
        <div className="mt-5 flex justify-end">
          <button type="button" onClick={onClose} className="cyber-button" aria-label="Cancel lookup">
            <X size={14} /> Cancel
          </button>
        </div>
      </div>
    </div>
  );
}
