// Card corner (⋮) menu for a library card: Unmatch, Match (auto) and Lookup.
//
// The menu lives in the HoloCard `cornerMenu` slot, which sits above the
// badge layer and reserves its own top-right corner (see holo-cinema.css).
// The card itself is a role="button", so every click inside the menu stops
// propagation — otherwise opening the menu or picking an item would also
// select the card. The menu closes on select, on Escape, and on any pointer
// press outside it. When the card is busy (an unmatch/delete is running) or
// a metadata check is in flight, the button and every item are disabled, so
// a second action can never race the one already running.

import { useEffect, useRef, useState } from "react";
import type { JSX, MouseEvent as ReactMouseEvent } from "react";
import { MoreVertical, Search, Sparkles, Unlink } from "lucide-react";

export interface CardCornerMenuProps {
  /** Card title, used only for accessible labels. */
  itemTitle: string;
  /** True while the card has a check or a card action in flight. */
  disabled?: boolean;
  /** Existing unmatch handler (confirm + keep file). */
  onUnmatch: () => void;
  /** Existing auto-match handler (Check metadata). */
  onMatch: () => void;
  /** Opens the Lookup picker dialog for this card. */
  onLookup: () => void;
}

export default function CardCornerMenu({
  itemTitle,
  disabled = false,
  onUnmatch,
  onMatch,
  onLookup,
}: CardCornerMenuProps): JSX.Element {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const buttonRef = useRef<HTMLButtonElement | null>(null);

  useEffect(() => {
    if (!open) return;
    const handlePointerDown = (event: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handlePointerDown);
    return () => document.removeEventListener("mousedown", handlePointerDown);
  }, [open]);

  useEffect(() => {
    if (disabled) setOpen(false);
  }, [disabled]);

  const toggle = (event: ReactMouseEvent<HTMLButtonElement>) => {
    event.stopPropagation();
    if (disabled) return;
    setOpen((current) => !current);
  };

  const pick = (handler: () => void) => (event: ReactMouseEvent<HTMLButtonElement>) => {
    event.stopPropagation();
    setOpen(false);
    handler();
  };

  return (
    <div
      ref={rootRef}
      className="holo-card__corner-menu"
      onClick={(event) => event.stopPropagation()}
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          setOpen(false);
          buttonRef.current?.focus();
        }
      }}
    >
      <button
        ref={buttonRef}
        type="button"
        className="holo-card__corner-button"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={`More actions for ${itemTitle}`}
        title={`More actions for ${itemTitle}`}
        disabled={disabled}
        onClick={toggle}
      >
        <MoreVertical size={15} />
      </button>
      {open && (
        <div className="holo-card__menu" role="menu" aria-label={`Actions for ${itemTitle}`}>
          <button
            type="button"
            role="menuitem"
            className="holo-card__menu-item"
            disabled={disabled}
            onClick={pick(onUnmatch)}
          >
            <Unlink size={13} /> Unmatch
          </button>
          <button
            type="button"
            role="menuitem"
            className="holo-card__menu-item"
            disabled={disabled}
            onClick={pick(onMatch)}
          >
            <Search size={13} /> Match (auto)
          </button>
          <button
            type="button"
            role="menuitem"
            className="holo-card__menu-item"
            disabled={disabled}
            onClick={pick(onLookup)}
          >
            <Sparkles size={13} /> Lookup…
          </button>
        </div>
      )}
    </div>
  );
}
