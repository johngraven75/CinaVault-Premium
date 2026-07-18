import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { Cast } from "lucide-react";
import CastingTab from "./tabs/CastingTab";

export default function CastButton() {
  const [open, setOpen] = useState(false);
  const [sidebarNav, setSidebarNav] = useState<Element | null>(null);

  useEffect(() => {
    const findSidebar = () => setSidebarNav(document.querySelector(".cv-sidebar nav"));
    findSidebar();
    const observer = new MutationObserver(findSidebar);
    observer.observe(document.body, { childList: true, subtree: true });
    return () => observer.disconnect();
  }, []);

  const menuButton = (
    <button
      type="button"
      className="group relative mt-1 flex w-full items-center gap-3 overflow-hidden rounded-2xl px-3 py-3 text-left text-cv-subtext outline-none transition-colors hover:text-cv-text focus-visible:ring-2 focus-visible:ring-cv-accent/70"
      data-testid="cinavault-casting-menu"
      aria-label="Open Casting Center"
      title="Casting Center"
      onClick={() => setOpen(true)}
    >
      <span className="relative z-10 grid h-11 w-11 shrink-0 place-items-center rounded-2xl border border-white/10 bg-black/30 shadow-[inset_0_1px_0_rgba(255,255,255,0.14)]">
        <Cast size={20} />
      </span>
      <span className="relative z-10 min-w-0 flex-1">
        <span className="block truncate text-[14px] font-bold tracking-tight">Casting</span>
        <span className="block text-[9px] uppercase tracking-[0.24em] opacity-65">Nearby devices</span>
      </span>
    </button>
  );

  return (
    <>
      {sidebarNav ? createPortal(menuButton, sidebarNav) : null}
      {open ? <CastingTab onClose={() => setOpen(false)} /> : null}
    </>
  );
}
