import { invoke } from "@tauri-apps/api/core";
import type { LibraryEnrichmentResult, MediaItem } from "../store/appStore";
import {
  fetchEnrichmentJobStatus,
  pollEnrichmentJob,
  type EnrichmentJobStart,
} from "../utils/enrichmentJob";

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

const STARTUP_IDLE_DELAY_MS = 30_000;
const REPAIR_SAMPLE_SIZE = 96;

/**
 * Issue 1: defer work to true idle. `requestIdleCallback` fires when the
 * main thread is idle, or at the latest after `timeoutMs` — so a startup
 * pass still runs even on a busy machine, just never ahead of idle work.
 * Falls back to a plain timer where the API is unavailable (older
 * WebViews), preserving the previous timing.
 */
function scheduleWhenIdle(callback: () => void, timeoutMs: number): () => void {
  const idleWindow = window as Window & {
    requestIdleCallback?: (
      callback: () => void,
      options?: { timeout: number },
    ) => number;
    cancelIdleCallback?: (id: number) => void;
  };
  if (typeof idleWindow.requestIdleCallback === "function") {
    const id = idleWindow.requestIdleCallback(callback, { timeout: timeoutMs });
    return () => {
      idleWindow.cancelIdleCallback?.(id);
    };
  }
  const timer = window.setTimeout(callback, timeoutMs);
  return () => window.clearTimeout(timer);
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

    // Yield between provider batches so React/WebView input and paint work stays responsive.
    await new Promise<void>((resolve) => window.setTimeout(resolve, 50));
  }

  return repaired;
}

async function runCycle(
  options: AiMediaAutopilotOptions,
  reason: string,
): Promise<AiMediaAutopilotCycle | null> {
  if (!options.enabled()) return null;
  // Issue 1: never stack autopilot work on top of a manual enrichment job.
  // The whole cycle is skipped (not queued: re-running immediately after
  // the manual job would just stack again); the next interval or trigger
  // retries. A backend hiccup here must not kill the cycle, hence the
  // catch that treats an unreadable status as "no job running".
  const activeJob = await fetchEnrichmentJobStatus(invoke).catch(() => null);
  if (activeJob && activeJob.status === "running") {
    options.addStatusMessage(
      "AI Media Autopilot skipped: an enrichment job is already running (manual job takes precedence).",
    );
    return null;
  }
  // Issue 1: defer deferrable passes while the app is hidden. A hidden
  // WebView pass would compete with whatever the user is doing elsewhere;
  // the next interval retries when visible. Manual and source-added
  // triggers are explicit and are not deferred.
  if (
    (reason === "startup idle health pass" ||
      reason === "scheduled maintenance") &&
    typeof document !== "undefined" &&
    document.hidden
  ) {
    return null;
  }
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

    // Issue 1: run_library_enrichment now returns a job start immediately
    // instead of the finished report. A "busy" start means a manual job
    // began between the cycle-start check above and this step; skip the
    // enrichment step rather than stacking. Otherwise poll the job to
    // completion (5s cadence; the poll is async, so it never blocks UI
    // rendering) and report from the finished job's result.
    const enrichmentStart = await invokeSafely<EnrichmentJobStart>(
      "run_library_enrichment",
      { renameFiles: false },
      warnings,
    );
    if (enrichmentStart && enrichmentStart.status === "busy") {
      warnings.push("enrichment skipped: an enrichment job is already running");
    } else if (enrichmentStart) {
      try {
        const finished = await pollEnrichmentJob(invoke, enrichmentStart.job_id, {
          intervalMs: 5000,
        });
        const enrichment = finished.result as
          | LibraryEnrichmentResult
          | undefined;
        enriched = Boolean(enrichment);
        if (enrichment) {
          options.addStatusMessage(
            `Autopilot enrichment: ${enrichment.metadata_items_enriched || enrichment.metadata_updated || 0} items, ${enrichment.metadata_fields_updated || 0} fields, ${enrichment.posters_downloaded || 0} posters`,
          );
        }
      } catch (error) {
        warnings.push(`enrichment job: ${messageFromError(error)}`);
      }
    }

    await invokeSafely("purge_photo_items", undefined, warnings);

    // Never pull the entire library into the WebView for an automated repair pass.
    // A bounded sample avoids huge JSON serialization, memory pressure, and React updates.
    const repairItems =
      (await invokeSafely<MediaItem[]>(
        "get_media_items",
        { mediaType: null, limit: REPAIR_SAMPLE_SIZE, offset: 0 },
        warnings,
      )) || [];

    targetedRepairs = await repairMetadataCandidates(repairItems, warnings);

    const count = await invokeSafely<{ total?: number }>(
      "get_library_count",
      { mediaType: null },
      warnings,
    );
    libraryCount = Number(count?.total || 0);

    // Let the library screen refresh its own paginated view. Replacing the global
    // store with every media row was the main UI-freeze path in v1.0.06.
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
        ? `AI Media Autopilot completed with ${warnings.length} warning(s); ${libraryCount} records remain available`
        : `AI Media Autopilot completed: ${libraryCount} records indexed and ${targetedRepairs} targeted repairs applied`,
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
  let cancelStartup: (() => void) | null = null;

  const scheduleStartupPass = () => {
    if (cancelStartup !== null) return;
    // Issue 1: defer to true idle (or at most STARTUP_IDLE_DELAY_MS after
    // library-ready), instead of a blind 30s timer that could fire
    // mid-interaction.
    cancelStartup = scheduleWhenIdle(() => {
      cancelStartup = null;
      void runCycle(options, "startup idle health pass");
    }, STARTUP_IDLE_DELAY_MS);
  };

  // The library announces its first usable page before metadata automation begins.
  // A fallback keeps maintenance available when the user starts on another tab.
  window.addEventListener("cinavault:library-ready", scheduleStartupPass, {
    once: true,
  });
  const cancelFallbackStartup = scheduleWhenIdle(scheduleStartupPass, 90_000);

  const interval = window.setInterval(
    () => void runCycle(options, "scheduled maintenance"),
    intervalMinutes * 60 * 1000,
  );

  // A new source still waits for idle before starting a cycle, for the
  // same reason as the startup pass.
  const handleSourceAdded = () =>
    scheduleWhenIdle(
      () => void runCycle(options, "new media source detected"),
      STARTUP_IDLE_DELAY_MS,
    );
  const handleManualRun = () =>
    void runCycle(options, "manual autopilot request");

  window.addEventListener("cinavault:source-added", handleSourceAdded);
  window.addEventListener("cinavault:ai-autopilot-run", handleManualRun);

  return () => {
    cancelStartup?.();
    cancelFallbackStartup();
    window.clearInterval(interval);
    window.removeEventListener("cinavault:library-ready", scheduleStartupPass);
    window.removeEventListener("cinavault:source-added", handleSourceAdded);
    window.removeEventListener("cinavault:ai-autopilot-run", handleManualRun);
  };
}
