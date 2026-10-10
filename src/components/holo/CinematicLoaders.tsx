// Cinematic loading and status primitives. All motion is CSS keyframes on
// transform and opacity (see styles/holo-cinema.css), so it runs on the
// compositor and stops under prefers-reduced-motion.
import type { JSX } from "react";
import { beaconLabel, progressFraction, type BeaconStatus } from "../../utils/holoMotion";

export function OrbitalSpinner({
  size = 44,
  label = "Loading",
  className = "",
}: {
  size?: number;
  label?: string;
  className?: string;
}): JSX.Element {
  return (
    <span
      role="status"
      aria-label={label}
      className={`holo-orbital ${className}`}
      style={{ width: size, height: size }}
    >
      <span className="holo-orbital__ring holo-orbital__ring--outer" />
      <span className="holo-orbital__ring holo-orbital__ring--inner" />
      <span className="holo-orbital__core" />
    </span>
  );
}

/**
 * Progress bar that fills with a transform (scaleX), never width. Pass
 * `value`/`max` for determinate progress; omit them for an indeterminate sweep.
 */
export function HoloProgressBar({
  value,
  max,
  label,
  className = "",
}: {
  value?: number | null;
  max?: number | null;
  label: string;
  className?: string;
}): JSX.Element {
  const fraction = progressFraction(value, max);
  const indeterminate = fraction === null;
  return (
    <div
      role="progressbar"
      aria-label={label}
      aria-valuemin={indeterminate ? undefined : 0}
      aria-valuemax={indeterminate ? undefined : 100}
      aria-valuenow={indeterminate ? undefined : Math.round(fraction * 100)}
      className={`holo-progress ${indeterminate ? "is-indeterminate" : ""} ${className}`}
    >
      <span
        className="holo-progress__fill"
        style={indeterminate ? undefined : { transform: `scaleX(${fraction})` }}
      />
      <span className="holo-progress__glint" />
    </div>
  );
}

export function StatusBeacon({
  status,
  label,
  className = "",
}: {
  status: BeaconStatus;
  label?: string;
  className?: string;
}): JSX.Element {
  const text = label ?? beaconLabel(status);
  return (
    <span className={`holo-beacon is-${status} ${className}`} role="status" aria-label={text}>
      <span className="holo-beacon__dot" aria-hidden="true">
        <span className="holo-beacon__pulse" />
      </span>
      {label !== "" && <span className="holo-beacon__label">{text}</span>}
    </span>
  );
}

export function HoloSkeletonCard({ index = 0 }: { index?: number }): JSX.Element {
  return (
    <div
      className="holo-skeleton"
      aria-hidden="true"
      style={{ animationDelay: `${Math.min(index, 12) * 70}ms` }}
    >
      <span className="holo-skeleton__poster" />
      <span className="holo-skeleton__line" />
      <span className="holo-skeleton__line is-short" />
    </div>
  );
}
