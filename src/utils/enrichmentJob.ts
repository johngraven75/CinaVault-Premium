// CinaVault Premium — enrichment background-job client helpers (Issue 1).
//
// Backend contract (see src-tauri/src/enrichment_jobs.rs):
//   * `gather_adult_metadata`, `run_library_enrichment`, and the adult route
//     of `ai_query` no longer await the whole-library loop. They return an
//     `EnrichmentJobStart` immediately:
//       { job_id, kind, status: "started" | "busy", message }
//     `status: "busy"` means a job is already running; `job_id` is that
//     running job's id, so the caller polls it instead of starting a second
//     job.
//   * `get_enrichment_job_status` (arg `jobId`, optional — omit or pass
//     null for the latest job) returns an `EnrichmentJobStatus`:
//       { job_id, kind, status, current, total, percent, message,
//         result?, error? }
//     `status` is "idle" | "running" | "completed" | "failed" | "cancelled".
//     On "completed", `result` holds the full report the old synchronous
//     command used to return (`AdultMetadataReport` /
//     `LibraryEnrichmentReport`, same shape). On "failed"/"cancelled",
//     `error` explains why.
//   * `stop_ai_agent` aborts the running job; polling then observes
//     "cancelled" and `pollEnrichmentJob` rejects with that error.
//
// This module is deliberately dependency-free (no Tauri import): the
// `invoke` function is injected, which keeps the helpers unit-testable under
// plain Node and lets every caller (AI Diagnostics tab, autopilot) share
// one interpretation of the job shapes.

export interface EnrichmentJobStart {
  job_id: string;
  kind: string;
  status: string;
  message: string;
}

export interface EnrichmentJobStatus {
  job_id: string;
  kind: string;
  status: string;
  current: number;
  total: number;
  percent: number;
  message: string;
  result?: unknown;
  error?: string | null;
}

/** Minimal structural type for the injected Tauri `invoke`. */
export type EnrichmentJobInvoke = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>;

/**
 * True when a command result is a job start (fresh or busy), rather than a
 * finished report. Anything else — a report, a diagnostics object, null —
 * is not a job start.
 */
export function isEnrichmentJobStart(value: unknown): value is EnrichmentJobStart {
  if (!value || typeof value !== "object") return false;
  const record = value as Record<string, unknown>;
  return (
    typeof record.job_id === "string" &&
    typeof record.kind === "string" &&
    (record.status === "started" || record.status === "busy")
  );
}

/** Terminal job states: polling stops here. */
export function isTerminalEnrichmentJobStatus(status: string): boolean {
  return status === "completed" || status === "failed" || status === "cancelled";
}

/** Fetch one job status snapshot. Omit `jobId` for the latest job. */
export async function fetchEnrichmentJobStatus(
  invokeFn: EnrichmentJobInvoke,
  jobId?: string | null,
): Promise<EnrichmentJobStatus> {
  return invokeFn<EnrichmentJobStatus>("get_enrichment_job_status", {
    jobId: jobId ?? null,
  });
}

/**
 * Poll a job until it reaches a terminal state.
 *
 * Resolves with the terminal status on "completed" (its `result` is the
 * full report). Rejects with an Error on "failed"/"cancelled". The caller
 * owns navigation: this promise never blocks rendering — it is just a
 * chain of 1s-apart status fetches, so the user can switch tabs while a
 * job runs and the job keeps running server-side.
 */
export async function pollEnrichmentJob(
  invokeFn: EnrichmentJobInvoke,
  jobId: string,
  options?: {
    intervalMs?: number;
    onProgress?: (status: EnrichmentJobStatus) => void;
  },
): Promise<EnrichmentJobStatus> {
  const intervalMs = options?.intervalMs ?? 1000;
  for (;;) {
    const status = await fetchEnrichmentJobStatus(invokeFn, jobId);
    options?.onProgress?.(status);
    if (isTerminalEnrichmentJobStatus(status.status)) {
      if (status.status === "completed") return status;
      throw new Error(
        status.error || status.message || `Enrichment job ${status.status}`,
      );
    }
    await new Promise<void>((resolve) => setTimeout(resolve, intervalMs));
  }
}
