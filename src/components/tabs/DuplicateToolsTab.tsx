// CinaVault Premium — Duplicate Finder and Deleter
import React, { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CheckSquare, Copy, RefreshCw, Search, Square, Trash2, X } from "lucide-react";
import { useAppStore } from "../../store/appStore";

interface DuplicateItem {
  id: number;
  media_id: number;
  file_path: string;
  file_size?: number;
  title?: string;
}

interface DuplicateGroup {
  id: number;
  group_hash: string;
  items: DuplicateItem[];
}

export default function DuplicateToolsTab() {
  const { addStatusMessage } = useAppStore();
  const [groups, setGroups] = useState<DuplicateGroup[]>([]);
  const [selectedItems, setSelectedItems] = useState<Set<number>>(new Set());
  const [matchBy, setMatchBy] = useState("name_size");
  const [toleranceMb, setToleranceMb] = useState(0);
  const [busy, setBusy] = useState<string | null>(null);
  const duplicateCount = groups.reduce((total, group) => total + group.items.length, 0);
  const removableItems = useMemo(() => groups.flatMap((group) => group.items.slice(1)), [groups]);
  const selectedCount = selectedItems.size;
  const allRemovableSelected = removableItems.length > 0 && removableItems.every((item) => selectedItems.has(item.id));

  const loadGroups = async () => {
    const result = await invoke<DuplicateGroup[]>("get_duplicate_groups");
    setGroups(result);
  };

  useEffect(() => {
    void loadGroups().catch(() => setGroups([]));
  }, []);

  useEffect(() => {
    const visibleIds = new Set(groups.flatMap((group) => group.items.map((item) => item.id)));
    setSelectedItems((current) => {
      const pruned = new Set([...current].filter((id) => visibleIds.has(id)));
      return pruned.size === current.size ? current : pruned;
    });
  }, [groups]);

  const findDuplicates = async () => {
    setBusy("scan");
    try {
      const result = await invoke<{ groups_found: number; total_duplicates: number }>("find_duplicates", {
        matchBy,
        toleranceMb,
      });
      await loadGroups();
      addStatusMessage(`Duplicate scan complete: ${result.groups_found} groups, ${result.total_duplicates} duplicate rows`);
    } catch (e) {
      addStatusMessage(`Duplicate scan failed: ${e}`);
    } finally {
      setBusy(null);
    }
  };

  const removeDuplicate = async (item: DuplicateItem, deleteFile: boolean) => {
    setBusy(`remove:${item.id}`);
    try {
      await invoke("remove_duplicate", { itemId: item.id, deleteFile });
      await loadGroups();
      setSelectedItems((current) => {
        const next = new Set(current);
        next.delete(item.id);
        return next;
      });
      addStatusMessage(deleteFile ? `Duplicate file deleted: ${item.title || item.file_path}` : `Duplicate removed from library: ${item.title || item.file_path}`);
    } catch (e) {
      addStatusMessage(`Duplicate removal failed: ${e}`);
    } finally {
      setBusy(null);
    }
  };

  const toggleSelection = (item: DuplicateItem) => {
    setSelectedItems((current) => {
      const next = new Set(current);
      if (next.has(item.id)) {
        next.delete(item.id);
      } else {
        next.add(item.id);
      }
      return next;
    });
  };

  const selectAllDuplicates = () => {
    setSelectedItems(new Set(removableItems.map((item) => item.id)));
  };

  const clearSelection = () => {
    setSelectedItems(new Set());
  };

  const removeSelected = async (deleteFile: boolean, all: boolean = false) => {
    const itemIds = all ? removableItems.map((item) => item.id) : [...selectedItems];
    if (itemIds.length === 0) {
      addStatusMessage("No duplicate rows selected");
      return;
    }

    setBusy(deleteFile ? "bulk-delete" : "bulk-remove");
    try {
      const result = await invoke<{ removed: number; missing: number }>("remove_duplicates", { itemIds, deleteFile });
      await loadGroups();
      clearSelection();
      const action = deleteFile ? "deleted from disk and library" : "removed from library";
      addStatusMessage(`Duplicate cleanup complete: ${result.removed} ${action}${result.missing ? `, ${result.missing} already gone` : ""}`);
    } catch (e) {
      addStatusMessage(`Duplicate removal failed: ${e}`);
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="space-y-5">
      <div className="glass-panel p-5">
        <div className="flex items-center justify-between gap-3 flex-wrap mb-4">
          <h3 className="text-sm font-bold flex items-center gap-2">
            <Copy size={16} className="text-cv-accent" /> Duplicate Finder
          </h3>
          <div className="flex items-center gap-2 text-xs text-cv-subtext">
            <span>{groups.length} groups</span>
            <span>{duplicateCount} duplicate rows</span>
          </div>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-[220px_160px_1fr] gap-3">
          <div>
            <label className="section-label">Match Rule</label>
            <select value={matchBy} onChange={(e) => setMatchBy(e.target.value)} className="cv-select w-full">
              <option value="name_size">Name + Size</option>
              <option value="name">Name</option>
              <option value="size">Size</option>
              <option value="hash">Partial File Hash</option>
            </select>
          </div>
          <div>
            <label className="section-label">Tolerance MB</label>
            <input
              type="number"
              min={0}
              value={toleranceMb}
              onChange={(e) => setToleranceMb(Number(e.target.value) || 0)}
              className="cv-input w-full"
            />
          </div>
          <div className="flex items-end gap-2 flex-wrap">
            <button onClick={findDuplicates} disabled={busy === "scan"} className="cv-btn cv-btn-primary">
              {busy === "scan" ? <RefreshCw size={14} className="animate-spin" /> : <Search size={14} />}
              Find Duplicates
            </button>
            <button onClick={() => void loadGroups()} className="cv-btn cv-btn-secondary">
              <RefreshCw size={14} /> Refresh
            </button>
          </div>
        </div>

        {groups.length > 0 && (
          <div className="mt-4 pt-4 border-t border-white/10 flex items-center justify-between gap-3 flex-wrap">
            <div className="text-xs text-cv-subtext">
              {selectedCount} selected · {removableItems.length} removable duplicates
            </div>
            <div className="flex items-center gap-2 flex-wrap">
              <button
                onClick={selectAllDuplicates}
                disabled={removableItems.length === 0 || allRemovableSelected}
                className="cv-btn cv-btn-secondary text-xs"
              >
                <CheckSquare size={13} /> Select All
              </button>
              <button onClick={clearSelection} disabled={selectedCount === 0} className="cv-btn cv-btn-secondary text-xs">
                <X size={13} /> Clear
              </button>
              <button
                onClick={() => void removeSelected(false)}
                disabled={selectedCount === 0 || busy === "bulk-remove" || busy === "bulk-delete"}
                className="cv-btn cv-btn-secondary text-xs"
              >
                <Trash2 size={13} /> Remove Selected Rows
              </button>
              <button
                onClick={() => void removeSelected(true)}
                disabled={selectedCount === 0 || busy === "bulk-remove" || busy === "bulk-delete"}
                className="cv-btn cv-btn-danger text-xs"
              >
                <Trash2 size={13} /> Delete Selected Files
              </button>
              <button
                onClick={() => void removeSelected(true, true)}
                disabled={removableItems.length === 0 || busy === "bulk-remove" || busy === "bulk-delete"}
                className="cv-btn cv-btn-danger text-xs"
              >
                <Trash2 size={13} /> Delete All
              </button>
            </div>
          </div>
        )}
      </div>

      {groups.length === 0 ? (
        <div className="glass-panel p-10 text-center">
          <Copy size={42} className="mx-auto text-cv-subtext/30 mb-3" />
          <div className="text-sm font-semibold mb-1">No Duplicate Groups Loaded</div>
          <div className="text-xs text-cv-subtext">Run the finder to locate duplicate media rows and choose what to remove.</div>
        </div>
      ) : (
        <div className="space-y-3">
          {groups.map((group) => (
            <div key={group.id} className="glass-panel rounded-xl overflow-hidden">
              <div className="px-4 py-3 border-b border-white/5 flex items-center justify-between">
                <div className="text-xs font-semibold">Group {group.id}</div>
                <div className="text-[10px] text-cv-subtext">{group.items.length} items</div>
              </div>
              <div className="divide-y divide-white/5">
                {group.items.map((item, index) => (
                  <div key={item.id} className="px-4 py-3 grid grid-cols-1 lg:grid-cols-[1fr_150px_240px] gap-3 items-center">
                    <div className="min-w-0 flex items-start gap-3">
                      <button
                        type="button"
                        onClick={() => toggleSelection(item)}
                        disabled={index === 0}
                        title={index === 0 ? "Keep copy" : "Select duplicate"}
                        className="mt-0.5 text-cv-subtext hover:text-cv-text disabled:opacity-40 disabled:hover:text-cv-subtext"
                      >
                        {selectedItems.has(item.id) ? <CheckSquare size={16} /> : <Square size={16} />}
                      </button>
                      <div className="min-w-0">
                        <div className="text-sm font-semibold truncate">{item.title || "Untitled media"}</div>
                        <div className="text-[11px] text-cv-subtext truncate">{item.file_path}</div>
                        {index === 0 && <div className="text-[10px] text-cv-accent mt-1">Kept as group reference</div>}
                      </div>
                    </div>
                    <div className="text-xs text-cv-subtext">
                      {item.file_size ? `${(item.file_size / 1_048_576).toFixed(1)} MB` : "Unknown size"}
                    </div>
                    <div className="flex items-center gap-2 justify-start lg:justify-end">
                      <button
                        onClick={() => void removeDuplicate(item, false)}
                        disabled={busy === `remove:${item.id}`}
                        className="cv-btn cv-btn-secondary text-xs"
                      >
                        <Trash2 size={12} /> Remove Row
                      </button>
                      <button
                        onClick={() => void removeDuplicate(item, true)}
                        disabled={busy === `remove:${item.id}`}
                        className="cv-btn cv-btn-danger text-xs"
                      >
                        <Trash2 size={12} /> Delete File
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
