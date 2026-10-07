// Pure math behind the holographic card and cinematic loaders. Kept free of
// DOM and React so it can be unit tested and reused by any surface.

export interface HoloPointerState {
  /** Card rotation around the X axis, in degrees (pointer near top tilts back). */
  rotateX: number;
  /** Card rotation around the Y axis, in degrees (pointer near right turns right). */
  rotateY: number;
  /** Pointer position across the card, 0 (left) to 1 (right). */
  px: number;
  /** Pointer position down the card, 0 (top) to 1 (bottom). */
  py: number;
  /** Distance from the card centre, 0 (centre) to 1 (a corner). */
  intensity: number;
}

export const HOLO_REST: HoloPointerState = Object.freeze({
  rotateX: 0,
  rotateY: 0,
  px: 0.5,
  py: 0.5,
  intensity: 0,
});

export const DEFAULT_MAX_TILT_DEG = 11;

function clamp01(value: number): number {
  if (!Number.isFinite(value)) return 0.5;
  return Math.min(1, Math.max(0, value));
}

function round(value: number, digits = 3): number {
  const factor = 10 ** digits;
  return Math.round(value * factor) / factor || 0;
}

/**
 * Maps a pointer position inside a card's bounding box to the tilt and light
 * position of the holographic layers. Points outside the box are clamped to
 * its edge, and a zero-sized box returns the resting state.
 */
export function computeHoloPointer(
  clientX: number,
  clientY: number,
  rect: { left: number; top: number; width: number; height: number },
  maxTiltDeg: number = DEFAULT_MAX_TILT_DEG,
): HoloPointerState {
  if (!(rect.width > 0) || !(rect.height > 0)) return { ...HOLO_REST };
  const px = clamp01((clientX - rect.left) / rect.width);
  const py = clamp01((clientY - rect.top) / rect.height);
  const dx = px - 0.5;
  const dy = py - 0.5;
  const tilt = Math.max(0, maxTiltDeg);
  return {
    rotateX: round(-dy * 2 * tilt),
    rotateY: round(dx * 2 * tilt),
    px: round(px),
    py: round(py),
    intensity: round(Math.min(1, Math.hypot(dx, dy) / Math.SQRT1_2)),
  };
}

/** CSS custom properties the holographic stylesheet reads. */
export function holoPointerStyleVars(state: HoloPointerState): Record<string, string> {
  return {
    "--holo-rx": `${state.rotateX}deg`,
    "--holo-ry": `${state.rotateY}deg`,
    "--holo-px": `${state.px}`,
    "--holo-py": `${state.py}`,
    "--holo-intensity": `${state.intensity}`,
  };
}

/**
 * Entrance delay for the Nth card in a grid. Only the first screenful
 * staggers; later cards (paged in while scrolling) appear without waiting.
 */
export function staggerDelay(index: number, step = 0.035, maxDelay = 0.42): number {
  if (!Number.isFinite(index) || index < 0) return 0;
  return round(Math.min(index * step, maxDelay));
}

/** Clamps a loaded/total pair to a 0..1 progress fraction, or null when unknown. */
export function progressFraction(done: number | null | undefined, total: number | null | undefined): number | null {
  if (done == null || total == null) return null;
  if (!Number.isFinite(done) || !Number.isFinite(total) || total <= 0) return null;
  return round(Math.min(1, Math.max(0, done / total)), 4);
}

export type BeaconStatus = "online" | "syncing" | "warning" | "offline";

const BEACON_LABELS: Record<BeaconStatus, string> = {
  online: "Online",
  syncing: "Syncing",
  warning: "Needs attention",
  offline: "Offline",
};

export function beaconLabel(status: BeaconStatus): string {
  return BEACON_LABELS[status] ?? BEACON_LABELS.offline;
}
