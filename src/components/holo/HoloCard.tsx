// Holographic media card: pointer-driven 3D tilt, parallax poster, foil
// shimmer and specular glare. Pointer tracking writes CSS custom properties
// once per animation frame (no React re-render, no layout reads per frame),
// and every animated property is a transform or opacity.
import { useCallback, useEffect, useRef } from "react";
import type { JSX, KeyboardEvent, PointerEvent, ReactNode } from "react";
import { motion, useReducedMotion } from "framer-motion";
import {
  computeHoloPointer,
  HOLO_REST,
  holoPointerStyleVars,
  staggerDelay,
} from "../../utils/holoMotion";

export interface HoloCardProps {
  /** Poster artwork (an <img> or a fallback), rendered on the parallax layer. */
  media: ReactNode;
  /** Badges that float above the artwork (copy count, verified, favorite). */
  badges?: ReactNode;
  /** Title block under the artwork. */
  info: ReactNode;
  /** Actions revealed on hover or keyboard focus. */
  actions?: ReactNode;
  /** Accessible name for the card. */
  label: string;
  /** Grid position, used to stagger the entrance of the first screenful. */
  index?: number;
  selected?: boolean;
  onSelect?: () => void;
  className?: string;
}

function applyVars(element: HTMLElement, vars: Record<string, string>): void {
  for (const [name, value] of Object.entries(vars)) {
    element.style.setProperty(name, value);
  }
}

export default function HoloCard({
  media,
  badges,
  info,
  actions,
  label,
  index = 0,
  selected = false,
  onSelect,
  className = "",
}: HoloCardProps): JSX.Element {
  const reduceMotion = useReducedMotion();
  const bodyRef = useRef<HTMLDivElement | null>(null);
  const rectRef = useRef<DOMRect | null>(null);
  const frameRef = useRef<number | null>(null);
  const pointerRef = useRef<{ x: number; y: number } | null>(null);

  const cancelFrame = () => {
    if (frameRef.current !== null) {
      cancelAnimationFrame(frameRef.current);
      frameRef.current = null;
    }
  };

  useEffect(() => cancelFrame, []);

  const flush = useCallback(() => {
    frameRef.current = null;
    const body = bodyRef.current;
    const rect = rectRef.current;
    const pointer = pointerRef.current;
    if (!body || !rect || !pointer) return;
    applyVars(body, holoPointerStyleVars(computeHoloPointer(pointer.x, pointer.y, rect)));
  }, []);

  const trackingDisabled = (event: PointerEvent<HTMLElement>) =>
    reduceMotion || event.pointerType === "touch";

  const handlePointerEnter = (event: PointerEvent<HTMLElement>) => {
    const body = bodyRef.current;
    if (!body || trackingDisabled(event)) return;
    // Measure once per hover, not per frame, so tracking never forces layout.
    rectRef.current = body.getBoundingClientRect();
    body.classList.add("is-tracking");
  };

  const handlePointerMove = (event: PointerEvent<HTMLElement>) => {
    if (!rectRef.current || trackingDisabled(event)) return;
    pointerRef.current = { x: event.clientX, y: event.clientY };
    if (frameRef.current === null) frameRef.current = requestAnimationFrame(flush);
  };

  const handlePointerLeave = () => {
    cancelFrame();
    rectRef.current = null;
    pointerRef.current = null;
    const body = bodyRef.current;
    if (!body) return;
    body.classList.remove("is-tracking");
    applyVars(body, holoPointerStyleVars(HOLO_REST));
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (event.target !== event.currentTarget) return;
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      onSelect?.();
    }
  };

  return (
    <motion.div
      role="button"
      tabIndex={0}
      aria-label={label}
      aria-pressed={selected}
      className={`holo-card ${selected ? "is-selected" : ""} ${className}`}
      initial={reduceMotion ? false : { opacity: 0, y: 22, scale: 0.94 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      transition={{
        duration: 0.55,
        ease: [0.16, 1, 0.3, 1],
        delay: reduceMotion ? 0 : staggerDelay(index),
      }}
      onClick={onSelect}
      onKeyDown={handleKeyDown}
      onPointerEnter={handlePointerEnter}
      onPointerMove={handlePointerMove}
      onPointerLeave={handlePointerLeave}
    >
      <div ref={bodyRef} className="holo-card__body">
        <div className="holo-card__media">{media}</div>
        <div className="holo-card__vignette" aria-hidden="true" />
        <div className="holo-card__foil" aria-hidden="true" />
        <div className="holo-card__glare" aria-hidden="true" />
        <div className="holo-card__scan" aria-hidden="true" />
        {badges && <div className="holo-card__badges">{badges}</div>}
        <div className="holo-card__info">{info}</div>
        {actions && <div className="holo-card__actions">{actions}</div>}
        <div className="holo-card__edge" aria-hidden="true" />
      </div>
    </motion.div>
  );
}
