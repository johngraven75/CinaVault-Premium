// Unified library UI pieces: the "×N copies" card badge and the copy picker
// shown in item detail panels so the user can choose which file to play.
import type { JSX } from "react";
import { Layers, Play } from "lucide-react";
import type { MediaCopyInfo, MediaItem } from "../../store/appStore";
import {
  copyCountLabel,
  copyFileName,
  formatFileSize,
  hasMultipleCopies,
} from "../../services/unifiedLibrary";

export function CopyCountBadge({
  item,
  className = "",
}: {
  item: Pick<MediaItem, "copy_count" | "copies">;
  className?: string;
}): JSX.Element | null {
  if (!hasMultipleCopies(item)) return null;
  const label = copyCountLabel(item);
  return (
    <span
      className={`cv-copy-badge inline-flex items-center gap-1 rounded-full border border-fuchsia-300/45 bg-fuchsia-500/25 px-2 py-0.5 text-[9px] font-black uppercase tracking-[0.08em] text-fuchsia-50 shadow-[0_0_14px_rgba(217,70,239,0.35)] backdrop-blur ${className}`}
      title={`${label} of this title across your libraries`}
      aria-label={label}
    >
      <Layers size={10} aria-hidden="true" /> {label}
    </span>
  );
}

export function CopiesList({
  item,
  onPlayCopy,
  className = "",
}: {
  item: Pick<MediaItem, "copy_count" | "copies" | "title" | "file_path">;
  onPlayCopy: (copy: MediaCopyInfo) => void;
  className?: string;
}): JSX.Element | null {
  const copies = item.copies ?? [];
  if (!hasMultipleCopies(item) || copies.length === 0) return null;

  return (
    <section
      className={`mt-4 border-t border-white/10 pt-3 ${className}`}
      aria-label={`Copies of ${item.title}`}
    >
      <div className="mb-2 flex items-center justify-between">
        <span className="flex items-center gap-1.5 text-[10px] font-bold uppercase tracking-[0.16em] text-cyan-100">
          <Layers size={12} aria-hidden="true" /> Copies
        </span>
        <span className="text-[10px] text-cv-subtext">
          {copies.length} file{copies.length === 1 ? "" : "s"} · choose one to play
        </span>
      </div>
      <ul className="space-y-1.5">
        {copies.map((copy, index) => (
          <li
            key={`${copy.id}-${copy.file_path}`}
            className="flex items-center gap-2 rounded-lg border border-white/10 bg-black/30 px-2.5 py-2"
          >
            <div className="min-w-0 flex-1">
              <div
                className="truncate text-[11px] font-semibold text-cv-text"
                title={copy.file_path}
              >
                {copyFileName(copy)}
                {index === 0 && (
                  <span className="ml-1.5 rounded bg-cyan-300/15 px-1 py-px text-[8px] font-bold uppercase tracking-wider text-cyan-200">
                    Best
                  </span>
                )}
              </div>
              <div className="mt-0.5 flex flex-wrap gap-x-2 text-[9px] uppercase tracking-[0.06em] text-cv-subtext">
                <span>{copy.resolution || "Unknown resolution"}</span>
                <span>{formatFileSize(copy.file_size)}</span>
              </div>
              <div
                className="mt-0.5 truncate text-[9px] text-cv-subtext/70"
                title={copy.file_path}
              >
                {copy.file_path}
              </div>
            </div>
            <button
              type="button"
              onClick={(event) => {
                event.stopPropagation();
                onPlayCopy(copy);
              }}
              className="cv-btn cv-btn-secondary shrink-0 px-2 py-1 text-[10px]"
              title={`Play ${copyFileName(copy)}`}
            >
              <Play size={11} aria-hidden="true" /> Play
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}
