// The row under a shelf heading. With Shelf Carousel Mode on it is one
// horizontal row with scroll buttons, wheel scrolling and drag to scroll;
// with it off the cards wrap into a grid.
import { useCallback, useEffect, useRef, useState } from "react";
import type { JSX, PointerEvent as ReactPointerEvent, ReactNode } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";

import { isFeatureOn } from "../../features/featureFlags";
import { carouselStep, wheelScrollDelta } from "../../services/discovery";
import { useAppStore } from "../../store/appStore";

const DRAG_THRESHOLD = 6;

export function useShelfCarousel(): boolean {
  return useAppStore((state) => isFeatureOn(state.featureSettings, "shelf_carousel"));
}

export default function ShelfRow({
  label,
  children,
  rowClassName = "",
  gridClassName = "",
}: {
  /** Accessible name of the row (the shelf title). */
  label: string;
  children: ReactNode;
  /** Extra classes for the carousel track (e.g. the skin's own shelf class). */
  rowClassName?: string;
  /** Extra classes for the wrapped grid. */
  gridClassName?: string;
}): JSX.Element {
  const carousel = useShelfCarousel();
  const trackRef = useRef<HTMLDivElement | null>(null);
  const dragRef = useRef<{ x: number; left: number; id: number; moved: boolean } | null>(null);
  const suppressClickRef = useRef(false);
  const [edges, setEdges] = useState({ start: true, end: true });

  const measure = useCallback(() => {
    const track = trackRef.current;
    if (!track) return;
    const max = track.scrollWidth - track.clientWidth;
    setEdges({ start: track.scrollLeft <= 1, end: track.scrollLeft >= max - 1 });
  }, []);

  useEffect(() => {
    const track = trackRef.current;
    if (!carousel || !track) return;
    measure();
    // Wheel needs a non-passive listener to keep the page from scrolling too.
    const onWheel = (event: WheelEvent) => {
      const delta = wheelScrollDelta(event.deltaX, event.deltaY, track.scrollLeft, track.scrollWidth - track.clientWidth);
      if (!delta) return;
      event.preventDefault();
      track.scrollLeft += delta;
    };
    track.addEventListener("wheel", onWheel, { passive: false });
    track.addEventListener("scroll", measure, { passive: true });
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(measure);
    observer?.observe(track);
    return () => {
      track.removeEventListener("wheel", onWheel);
      track.removeEventListener("scroll", measure);
      observer?.disconnect();
    };
  }, [carousel, measure, children]);

  const scrollBy = (direction: 1 | -1) => {
    const track = trackRef.current;
    if (!track) return;
    const card = track.firstElementChild as HTMLElement | null;
    const gap = parseFloat(getComputedStyle(track).columnGap) || 0;
    track.scrollBy({ left: direction * carouselStep(track.clientWidth, card?.offsetWidth ?? 160, gap), behavior: "smooth" });
  };

  const onPointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.pointerType !== "mouse" || event.button !== 0) return;
    dragRef.current = { x: event.clientX, left: event.currentTarget.scrollLeft, id: event.pointerId, moved: false };
  };
  const onPointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.id !== event.pointerId) return;
    const dx = event.clientX - drag.x;
    if (!drag.moved && Math.abs(dx) < DRAG_THRESHOLD) return;
    if (!drag.moved) {
      drag.moved = true;
      event.currentTarget.setPointerCapture(event.pointerId);
    }
    event.currentTarget.scrollLeft = drag.left - dx;
  };
  const endDrag = (event: ReactPointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.id !== event.pointerId) return;
    suppressClickRef.current = drag.moved;
    dragRef.current = null;
  };

  if (!carousel) {
    return (
      <div className={`cv-shelf-grid ${gridClassName}`} role="group" aria-label={label}>
        {children}
      </div>
    );
  }

  return (
    <div className="cv-shelf-carousel">
      <button
        type="button"
        className="cv-shelf-scroll is-prev"
        onClick={() => scrollBy(-1)}
        disabled={edges.start}
        aria-label={`Scroll ${label} left`}
      >
        <ChevronLeft size={18} />
      </button>
      <div
        ref={trackRef}
        className={`cv-shelf-track ${rowClassName}`}
        role="group"
        aria-label={label}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={endDrag}
        onPointerCancel={endDrag}
        onClickCapture={(event) => {
          // A drag that ends on a card is not a click on it.
          if (suppressClickRef.current) {
            suppressClickRef.current = false;
            event.stopPropagation();
            event.preventDefault();
          }
        }}
      >
        {children}
      </div>
      <button
        type="button"
        className="cv-shelf-scroll is-next"
        onClick={() => scrollBy(1)}
        disabled={edges.end}
        aria-label={`Scroll ${label} right`}
      >
        <ChevronRight size={18} />
      </button>
    </div>
  );
}
