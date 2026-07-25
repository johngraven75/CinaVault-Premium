import { invoke } from "@tauri-apps/api/core";
import type { LibraryEnrichmentResult, MediaItem } from "../store/appStore";

export interface AiMediaAutopilotOptions {
  enabled: () => boolean;
  addStatusMessage: (message: string) => void;
  setMediaItems: (items: MediaItem[]) => void;
  intervalMinutes?: number;
}

export interface AiMediaAutopilotCycle {
  startedAt: string;
  finishedAt: string;
  scanned: boolean;
  enriched: boolean;
  targetedRepairs: number;
  libraryCount: number;
  warnings: string[];
}

let cycleRunning = false;
let cycleQueued = false;

function messageFromError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

async function invokeSafely<T>(
  command: string,
  args: Record<string, unknown> | undefined,
  warnings: string[],
): Promise<T | null> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    warnings.push(`${command}: ${messageFromError(error)}`);
    return null;
  }
}

function mediaNeedsRepair(item: MediaItem): boolean {
  return Boolean(
    item.id &&
      (!item.poster_path ||
        !item.overview ||
        !item.year ||
        !item.genre ||
        !item.verified),
  );
}

async function repairMetadataCandidates(
  items: MediaItem[],
  warnings: string[],
): Promise<number> {
  const candidates = items.filter(mediaNeedsRepair).slice(0, 24);
  let repaired = 0;

  for (let index = 0; index < candidates.length; index += 4) {
    const group = candidates.slice(index, index + 4);
    const results = await Promise.allSettled(
      group.map((item) =>
        invoke<{ metadata_updated?: boolean }>("check_media_item_metadata", {
          id: item.id,
        }),
      ),
    );
    results.forEach((result, resultIndex) => {
      if (result.status === "fulfilled") {
        if (result.value?.metadata_updated) repaired += 1;
      } else {
        warnings.push(
          `metadata repair ${group[resultIndex]?.title || "unknown"}: ${messageFromError(result.reason)}`,
        );
      }
    });
  }

  return repaired;
}

async function runCycle(
  options: AiMediaAutopilotOptions,
  reason: string,
): Promise<AiMediaAutopilotCycle | null> {
  if (!options.enabled()) return null;
  if (cycleRunning) {
    cycleQueued = true;
    return null;
  }

  cycleRunning = true;
  const warnings: string[] = [];
  const startedAt = new Date().toISOString();
  let scanned = false;
  let enriched = false;
  let targetedRepairs = 0;
  let libraryCount = 0;

  options.addStatusMessage(`AI Media Autopilot started: ${reason}`);

  try {
    const scan = await invokeSafely<{
      total_found?: number;
      total_added?: number;
      total_updated?: number;
      sources_scanned?: number;
    }>("scan_sources", undefined, warnings);
    scanned = Boolean(scan);
    if (scan) {
      options.addStatusMessage(
        `Autopilot scan: ${scan.total_found || 0} found, ${scan.total_added || 0} new, ${scan.total_updated || 0} refreshed`,
      );
    }

    const enrichment = await invokeSafely<LibraryEnrichmentResult>(
      "run_library_enrichment",
      { renameFiles: false },
      warnings,
    );
    enriched = Boolean(enrichment);
    if (enrichment) {
      options.addStatusMessage(
        `Autopilot enrichment: ${enrichment.metadata_items_enriched || enrichment.metadata_updated || 0} items, ${enrichment.metadata_fields_updated || 0} fields, ${enrichment.posters_downloaded || 0} posters`,
      );
    }

    await invokeSafely("purge_photo_items", undefined, warnings);

    const initialItems =
      (await invokeSafely<MediaItem[]>(
        "get_media_items",
        { mediaType: null, limit: null, offset: null },
        warnings,
      )) || [];

    targetedRepairs = await repairMetadataCandidates(initialItems, warnings);

    const finalItems =
      (await invokeSafely<MediaItem[]>(
        "get_media_items",
        { mediaType: null, limit: null, offset: null },
        warnings,
      )) || initialItems;
    libraryCount = finalItems.length;
    options.setMediaItems(finalItems);

    window.dispatchEvent(
      new CustomEvent("cinavault:library-refresh", {
        detail: {
          reason: "ai-media-autopilot",
          libraryCount,
          targetedRepairs,
        },
      }),
    );

    options.addStatusMessage(
      warnings.length
        ? `AI Media Autopilot completed with ${warnings.length} warning(s); ${libraryCount} cards refreshed`
        : `AI Media Autopilot completed: ${libraryCount} cards refreshed and ${targetedRepairs} targeted repairs applied`,
    );
  } finally {
    cycleRunning = false;
  }

  const result: AiMediaAutopilotCycle = {
    startedAt,
    finishedAt: new Date().toISOString(),
    scanned,
    enriched,
    targetedRepairs,
    libraryCount,
    warnings,
  };

  if (cycleQueued) {
    cycleQueued = false;
    window.setTimeout(() => void runCycle(options, "queued library change"), 1500);
  }

  return result;
}

export function startAiMediaAutopilot(
  options: AiMediaAutopilotOptions,
): () => void {
  const intervalMinutes = Math.max(10, options.intervalMinutes || 30);
  const startupTimer = window.setTimeout(
    () => void runCycle(options, "startup health pass"),
    4500,
  );
  const interval = window.setInterval(
    () => void runCycle(options, "scheduled maintenance"),
    intervalMinutes * 60 * 1000,
  );

  const handleSourceAdded = () =>
    void runCycle(options, "new media source detected");
  const handleManualRun = () =>
    void runCycle(options, "manual autopilot request");

  window.addEventListener("cinavault:source-added", handleSourceAdded);
  window.addEventListener("cinavault:ai-autopilot-run", handleManualRun);

  return () => {
    window.clearTimeout(startupTimer);
    window.clearInterval(interval);
    window.removeEventListener("cinavault:source-added", handleSourceAdded);
    window.removeEventListener("cinavault:ai-autopilot-run", handleManualRun);
  };
}
