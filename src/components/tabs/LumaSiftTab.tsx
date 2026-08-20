import { invoke } from "@tauri-apps/api/core";
import { AnimatePresence, motion } from "framer-motion";
import {
  Aperture,
  BadgeCheck,
  CircleAlert,
  Clock3,
  Eye,
  FileAudio,
  FileImage,
  FileText,
  FileVideo2,
  FolderCheck,
  HardDrive,
  LoaderCircle,
  ScanSearch,
  ShieldCheck,
  Sparkles,
  Trash2,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useAppStore } from "../../store/appStore";

type LumaSiftProgress = {
  scanning: boolean;
  phase: string;
  current: number;
  total: number;
  percentage: number;
  current_path?: string | null;
  files_considered: number;
  message: string;
  error?: string | null;
};

type QualityEvidence = {
  width?: number | null;
  height?: number | null;
  pixel_count: number;
  bitrate?: number | null;
  bit_depth?: number | null;
  duration_millis?: number | null;
  file_size_bytes: number;
  reasons: string[];
};

type Candidate = {
  id: string;
  file_path: string;
  display_name: string;
  media_kind: string;
  quality_score: number;
  quality: QualityEvidence;
  disposition: string;
  disposition_detail: string;
  quarantine_path?: string | null;
};

type Group = {
  id: string;
  exact_hash: string;
  winner_id: string;
  reclaimable_bytes: number;
  candidates: Candidate[];
};

type Disposition = {
  occurred_at: string;
  file_path: string;
  display_name: string;
  disposition: string;
  detail: string;
};

type Plan = {
  id: string;
  status: string;
  created_at: string;
  groups: Group[];
  reclaimable_bytes: number;
  queued_file_count: number;
  dispositions: Disposition[];
};

type SelectionType = "video" | "audio" | "document" | "image";

const RESOLUTION_TYPES: Array<{ id: SelectionType; label: string; detail: string; icon: typeof FileVideo2; accent: string }> = [
  { id: "video", label: "Videos", detail: "MP4, MKV, MOV and more", icon: FileVideo2, accent: "text-cyan-100" },
  { id: "audio", label: "MP3 audio", detail: "MP3 files only", icon: FileAudio, accent: "text-violet-100" },
  { id: "document", label: "Documents", detail: "DOCX and PDF", icon: FileText, accent: "text-amber-100" },
  { id: "image", label: "Images", detail: "JPG, PNG, HEIC and more", icon: FileImage, accent: "text-fuchsia-100" },
];

const EMPTY_PROGRESS: LumaSiftProgress = {
  scanning: false,
  phase: "Ready",
  current: 0,
  total: 0,
  percentage: 0,
  files_considered: 0,
  message: "Build a read-only exact-duplicate plan for your indexed videos and photos.",
};

function formatBytes(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const index = Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1);
  return `${(value / 1024 ** index).toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
}

function candidateIcon(kind: string) {
  return kind === "image" || kind === "photo" ? FileImage : FileVideo2;
}

function dispositionTone(value: string): string {
  if (value === "retain") return "border-cyan-300/30 bg-cyan-300/10 text-cyan-100";
  if (value === "quarantined") return "border-violet-300/30 bg-violet-300/10 text-violet-100";
  if (value === "failed") return "border-rose-300/35 bg-rose-300/10 text-rose-100";
  if (value === "queued_for_quarantine") return "border-amber-300/30 bg-amber-300/10 text-amber-100";
  return "border-white/12 bg-white/[0.05] text-slate-200";
}

function displayDisposition(value: string): string {
  return value.replaceAll("_", " ");
}

export default function LumaSiftTab() {
  const addStatusMessage = useAppStore((state) => state.addStatusMessage);
  const [progress, setProgress] = useState<LumaSiftProgress>(EMPTY_PROGRESS);
  const [plan, setPlan] = useState<Plan | null>(null);
  const [selectedTypes, setSelectedTypes] = useState<SelectionType[]>(RESOLUTION_TYPES.map((type) => type.id));
  const [loading, setLoading] = useState(true);
  const [applying, setApplying] = useState(false);
  const [confirmingPlan, setConfirmingPlan] = useState(false);
  const [showPurge, setShowPurge] = useState(false);
  const [purgePhrase, setPurgePhrase] = useState("");

  const loadState = async () => {
    try {
      const [nextProgress, nextPlan] = await Promise.all([
        invoke<LumaSiftProgress>("get_lumasift_progress"),
        invoke<Plan | null>("get_lumasift_plan"),
      ]);
      setProgress(nextProgress);
      setPlan(nextPlan);
    } catch (error) {
      addStatusMessage(`LumaSift status is unavailable: ${error}`);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void loadState();
    const poll = window.setInterval(() => { void loadState(); }, 700);
    return () => window.clearInterval(poll);
  }, []);

  const toggleType = (type: SelectionType) => {
    setSelectedTypes((current) => current.includes(type) ? current.filter((value) => value !== type) : [...current, type]);
  };

  const startResolution = async () => {
    if (selectedTypes.length === 0) {
      addStatusMessage("Choose at least one file type before starting LumaSift.");
      return;
    }
    setLoading(true);
    try {
      const response = await invoke<{ message: string }>("start_lumasift_resolution", { selectedTypes });
      addStatusMessage(response.message);
      await loadState();
    } catch (error) {
      addStatusMessage(`LumaSift could not start: ${error}`);
      setLoading(false);
    }
  };

  const cancelResolution = async () => {
    try {
      await invoke("cancel_lumasift_resolution");
      addStatusMessage("LumaSift cancellation requested; no media files have been changed.");
    } catch (error) {
      addStatusMessage(`LumaSift cancellation could not be requested: ${error}`);
    }
  };

  const applyPlan = async () => {
    if (!plan) return;
    setApplying(true);
    try {
      const response = await invoke<{ message: string }>("apply_lumasift_plan", { planId: plan.id });
      addStatusMessage(response.message);
      setConfirmingPlan(false);
      await loadState();
    } catch (error) {
      addStatusMessage(`LumaSift could not apply the quarantine plan: ${error}`);
    } finally {
      setApplying(false);
    }
  };

  const purgeQuarantine = async () => {
    setApplying(true);
    try {
      const response = await invoke<{ message?: string; erased?: number }>("purge_lumasift_quarantine", { confirmation: purgePhrase });
      addStatusMessage(response.message ?? `LumaSift permanently erased ${response.erased ?? 0} quarantined files.`);
      setShowPurge(false);
      setPurgePhrase("");
      await loadState();
    } catch (error) {
      addStatusMessage(`Permanent erase did not run: ${error}`);
    } finally {
      setApplying(false);
    }
  };

  const queued = useMemo(() => plan?.groups.flatMap((group) => group.candidates).filter((candidate) => candidate.disposition === "queued_for_quarantine") ?? [], [plan]);
  const planIsActionable = plan?.status === "ready_for_review" && queued.length > 0;
  const percent = Math.max(0, Math.min(100, progress.percentage || 0));

  return (
    <div className="space-y-5">
      <section className="relative overflow-hidden rounded-[28px] border border-cyan-200/20 bg-[linear-gradient(135deg,rgba(8,15,36,0.98),rgba(29,7,58,0.95)_48%,rgba(75,8,68,0.9))] p-6 shadow-[0_24px_80px_rgba(20,0,60,0.33)]">
        <div className="pointer-events-none absolute -right-24 -top-24 h-72 w-72 rounded-full bg-fuchsia-500/20 blur-3xl" />
        <div className="pointer-events-none absolute -left-24 bottom-0 h-72 w-72 rounded-full bg-cyan-400/16 blur-3xl" />
        <div className="relative flex flex-col gap-5 xl:flex-row xl:items-center xl:justify-between">
          <div className="flex items-center gap-4">
            <div className="grid h-20 w-20 shrink-0 place-items-center overflow-hidden rounded-[23px] border border-white/20 bg-black/40 shadow-[0_0_36px_rgba(50,216,255,0.28)]">
              <img src="/branding/lumasift/lumasift-prism.png" alt="LumaSift prism mark" className="h-full w-full object-cover" />
            </div>
            <div>
              <div className="flex items-center gap-2 text-[10px] font-black uppercase tracking-[0.28em] text-cyan-100"><Aperture size={13} /> Exact media resolution</div>
              <h3 className="mt-1 text-3xl font-black tracking-[-0.05em] text-white">LumaSift</h3>
              <p className="mt-1 max-w-2xl text-sm text-slate-300">Keep the luminous best copy. LumaSift proves exact matches, explains its quality choice, and moves lower-ranked copies to a recoverable quarantine.</p>
            </div>
          </div>
          <div className="flex flex-wrap gap-2">
            <button type="button" onClick={() => void startResolution()} disabled={progress.scanning || loading || applying || selectedTypes.length === 0} className="cv-btn cv-btn-primary disabled:opacity-50"><ScanSearch size={15} className={progress.scanning ? "animate-spin" : ""} /> {progress.scanning ? "Mapping duplicates..." : "Build exact plan"}</button>
            {progress.scanning && <button type="button" onClick={() => void cancelResolution()} className="cv-btn cv-btn-secondary"><CircleAlert size={15} /> Cancel safely</button>}
            <button type="button" onClick={() => setShowPurge(true)} disabled={progress.scanning || applying} className="cv-btn cv-btn-danger disabled:opacity-50"><Trash2 size={15} /> Erase quarantine</button>
          </div>
        </div>
      </section>

      <section className="glass-panel p-5">
        <div className="mb-3 flex flex-wrap items-center justify-between gap-3"><div><h3 className="flex items-center gap-2 text-sm font-bold"><Eye size={16} className="text-cyan-100" /> Choose file types to resolve</h3><p className="mt-1 text-xs text-cv-subtext">LumaSift scans only the selected indexed types. Each category uses exact-content proof before it can enter a resolution plan.</p></div><span className="rounded-full border border-white/10 bg-black/20 px-3 py-1 text-xs font-bold text-white">{selectedTypes.length} selected</span></div>
        <div className="grid grid-cols-1 gap-2 sm:grid-cols-2 xl:grid-cols-4">{RESOLUTION_TYPES.map((type) => { const Icon = type.icon; const active = selectedTypes.includes(type.id); return <button key={type.id} type="button" onClick={() => toggleType(type.id)} className={`flex items-center gap-3 rounded-2xl border p-3 text-left transition ${active ? "border-cyan-200/35 bg-cyan-300/[0.09]" : "border-white/10 bg-black/15 opacity-65 hover:opacity-100"}`}><span className={`grid h-9 w-9 place-items-center rounded-xl bg-black/25 ${type.accent}`}><Icon size={17} /></span><span className="min-w-0 flex-1"><span className="block text-xs font-black text-white">{type.label}</span><span className="block truncate text-[10px] text-cv-subtext">{type.detail}</span></span><span className={`h-4 w-4 rounded-full border ${active ? "border-cyan-100 bg-cyan-300 shadow-[0_0_12px_rgba(103,232,249,0.8)]" : "border-white/25"}`} /></button>; })}</div>
        {selectedTypes.length === 0 && <p className="mt-3 text-xs text-amber-100">Choose one or more categories to enable the exact-plan scan.</p>}
      </section>

      <section className="glass-panel p-5">
        <div className="mb-4 flex flex-wrap items-center justify-between gap-3"><div><div className="flex items-center gap-2 text-sm font-bold"><Sparkles size={16} className="text-fuchsia-200" /> {progress.phase}</div><p className="mt-1 text-xs text-cv-subtext">{progress.message}</p></div><div className="rounded-full border border-cyan-300/25 bg-cyan-300/10 px-3 py-1 text-sm font-black text-cyan-100">{percent}%</div></div>
        <div className="h-3 overflow-hidden rounded-full bg-black/30 ring-1 ring-white/10"><motion.div className="h-full rounded-full bg-[linear-gradient(90deg,#28D7FF,#7A3DFF_54%,#F51393)] shadow-[0_0_18px_rgba(52,211,255,0.75)]" animate={{ width: `${percent}%` }} transition={{ duration: 0.25 }} /></div>
        <div className="mt-3 grid gap-3 text-xs md:grid-cols-3"><div className="rounded-xl border border-white/10 bg-black/20 p-3"><span className="text-cv-subtext">Processed</span><div className="mt-1 font-mono text-sm text-white">{progress.current.toLocaleString()} / {progress.total.toLocaleString()}</div></div><div className="rounded-xl border border-white/10 bg-black/20 p-3"><span className="text-cv-subtext">Indexed media</span><div className="mt-1 font-mono text-sm text-white">{progress.files_considered.toLocaleString()}</div></div><div className="rounded-xl border border-white/10 bg-black/20 p-3"><span className="text-cv-subtext">Current file</span><div className="mt-1 truncate font-mono text-[11px] text-white" title={progress.current_path ?? "No active file"}>{progress.current_path ?? "No active file"}</div></div></div>
        {progress.error && <div className="mt-3 rounded-xl border border-rose-300/30 bg-rose-400/10 p-3 text-xs text-rose-100"><CircleAlert size={14} className="mr-2 inline" />{progress.error}</div>}
      </section>

      <section className="grid gap-4 lg:grid-cols-3">
        <div className="glass-panel p-5"><div className="flex items-center gap-2 text-xs font-black uppercase tracking-[0.14em] text-cyan-100"><BadgeCheck size={15} /> Exact groups</div><div className="mt-3 text-3xl font-black text-white">{plan?.groups.length ?? 0}</div><p className="mt-1 text-xs text-cv-subtext">Every group shares a complete SHA-256 digest.</p></div>
        <div className="glass-panel p-5"><div className="flex items-center gap-2 text-xs font-black uppercase tracking-[0.14em] text-amber-100"><HardDrive size={15} /> Recoverable space</div><div className="mt-3 text-3xl font-black text-white">{formatBytes(plan?.reclaimable_bytes ?? 0)}</div><p className="mt-1 text-xs text-cv-subtext">Available after approved files move to quarantine.</p></div>
        <div className="glass-panel p-5"><div className="flex items-center gap-2 text-xs font-black uppercase tracking-[0.14em] text-violet-100"><ShieldCheck size={15} /> Guarded action</div><div className="mt-3 text-3xl font-black text-white">{queued.length}</div><p className="mt-1 text-xs text-cv-subtext">Lower-ranked files awaiting your quarantine approval.</p></div>
      </section>

      <section className="glass-panel overflow-hidden rounded-2xl">
        <div className="flex flex-wrap items-center justify-between gap-3 border-b border-white/10 px-5 py-4"><div><h3 className="flex items-center gap-2 text-sm font-bold"><FolderCheck size={16} className="text-cyan-200" /> Resolution plan</h3><p className="mt-1 text-xs text-cv-subtext">The winner is retained. No plan changes a file until you approve quarantine.</p></div>{planIsActionable && <button type="button" onClick={() => setConfirmingPlan(true)} className="cv-btn cv-btn-gold"><FolderCheck size={15} /> Review {queued.length} quarantine moves</button>}</div>
        {!plan ? <div className="p-10 text-center"><Aperture size={42} className="mx-auto mb-3 text-cyan-200/30" /><p className="text-sm font-semibold">Your next clean edit starts here.</p><p className="mt-1 text-xs text-cv-subtext">Scan your selected sources first, then build a read-only exact-duplicate plan.</p></div> : plan.groups.length === 0 ? <div className="p-10 text-center"><BadgeCheck size={42} className="mx-auto mb-3 text-emerald-300/50" /><p className="text-sm font-semibold">No exact duplicates are in this plan.</p><p className="mt-1 text-xs text-cv-subtext">{plan.status === "cancelled" ? "The last operation was cancelled safely." : "Potential matches must share a full content digest before they appear here."}</p></div> : <div className="divide-y divide-white/[0.08]">{plan.groups.map((group, index) => <motion.article key={group.id} initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: Math.min(index * 0.03, 0.2) }} className="p-5"><div className="mb-4 flex flex-wrap items-center justify-between gap-3"><div><div className="text-xs font-bold uppercase tracking-[0.14em] text-cyan-100">Exact group {index + 1}</div><div className="mt-1 font-mono text-[10px] text-cv-subtext">SHA-256 {group.exact_hash.slice(0, 18)}…</div></div><div className="rounded-full border border-amber-300/25 bg-amber-300/10 px-3 py-1 text-xs font-bold text-amber-100">{formatBytes(group.reclaimable_bytes)} reclaimable</div></div><div className="space-y-2">{group.candidates.map((candidate) => { const Icon = candidateIcon(candidate.media_kind); const winner = candidate.id === group.winner_id; return <div key={candidate.id} className={`rounded-2xl border p-4 ${winner ? "border-cyan-200/35 bg-cyan-300/[0.08]" : "border-white/10 bg-black/15"}`}><div className="flex flex-wrap items-start gap-3"><div className={`grid h-9 w-9 shrink-0 place-items-center rounded-xl ${winner ? "bg-cyan-300/15 text-cyan-100" : "bg-fuchsia-300/10 text-fuchsia-100"}`}><Icon size={17} /></div><div className="min-w-0 flex-1"><div className="flex flex-wrap items-center gap-2"><span className="max-w-full truncate text-sm font-bold text-white" title={candidate.file_path}>{candidate.display_name}</span><span className={`rounded-full border px-2 py-0.5 text-[10px] font-black uppercase tracking-[0.1em] ${dispositionTone(candidate.disposition)}`}>{displayDisposition(candidate.disposition)}</span></div><p className="mt-1 truncate font-mono text-[10px] text-cv-subtext" title={candidate.file_path}>{candidate.file_path}</p><div className="mt-2 flex flex-wrap gap-1.5">{candidate.quality.reasons.slice(0, 4).map((reason) => <span key={reason} className="rounded-md bg-white/[0.06] px-2 py-1 text-[10px] text-slate-300">{reason}</span>)}</div><p className="mt-2 text-[11px] text-cv-subtext">{candidate.disposition_detail}</p></div><div className="ml-auto text-right"><div className="text-[10px] uppercase tracking-[0.13em] text-cv-subtext">Quality</div><div className="font-mono text-sm font-black text-white">{candidate.quality_score.toLocaleString()}</div></div></div></div>; })}</div></motion.article>)}</div>}
      </section>

      {plan && plan.dispositions.length > 0 && <section className="glass-panel overflow-hidden rounded-2xl"><div className="flex items-center gap-2 border-b border-white/10 px-5 py-4 text-sm font-bold"><Clock3 size={16} className="text-violet-200" /> Files & dispositions</div><div className="max-h-80 divide-y divide-white/[0.06] overflow-y-auto">{plan.dispositions.slice().reverse().map((event, index) => <div key={`${event.occurred_at}-${event.file_path}-${index}`} className="flex flex-wrap items-center gap-3 px-5 py-3 text-xs"><span className={`rounded-full border px-2 py-0.5 text-[10px] font-black uppercase tracking-[0.1em] ${dispositionTone(event.disposition)}`}>{displayDisposition(event.disposition)}</span><span className="min-w-0 flex-1 truncate font-semibold text-white" title={event.file_path}>{event.display_name}</span><span className="max-w-lg truncate text-cv-subtext" title={event.detail}>{event.detail}</span></div>)}</div></section>}

      <AnimatePresence>{confirmingPlan && plan && <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="fixed inset-0 z-50 grid place-items-center bg-[#02030a]/80 p-4 backdrop-blur-sm"><motion.div initial={{ y: 18, scale: 0.98 }} animate={{ y: 0, scale: 1 }} exit={{ y: 18, scale: 0.98 }} className="w-full max-w-xl rounded-[26px] border border-amber-200/30 bg-[#11152b] p-6 shadow-2xl"><div className="flex items-center gap-3"><div className="grid h-12 w-12 place-items-center rounded-2xl bg-amber-300/15 text-amber-100"><FolderCheck /></div><div><h4 className="text-lg font-black">Approve quarantine plan?</h4><p className="text-xs text-slate-300">LumaSift will move—not permanently erase—{queued.length} lower-ranked exact duplicate files.</p></div></div><div className="mt-4 rounded-xl border border-white/10 bg-black/20 p-3 text-sm"><b>{formatBytes(plan.reclaimable_bytes)}</b> becomes recoverable once you later choose to empty quarantine. Each item is rehashed before it moves.</div><div className="mt-5 flex justify-end gap-2"><button type="button" onClick={() => setConfirmingPlan(false)} disabled={applying} className="cv-btn cv-btn-secondary">Keep reviewing</button><button type="button" onClick={() => void applyPlan()} disabled={applying} className="cv-btn cv-btn-gold disabled:opacity-50">{applying ? <LoaderCircle size={15} className="animate-spin" /> : <ShieldCheck size={15} />} Move to quarantine</button></div></motion.div></motion.div>}</AnimatePresence>

      <AnimatePresence>{showPurge && <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="fixed inset-0 z-50 grid place-items-center bg-[#02030a]/80 p-4 backdrop-blur-sm"><motion.div initial={{ y: 18, scale: 0.98 }} animate={{ y: 0, scale: 1 }} exit={{ y: 18, scale: 0.98 }} className="w-full max-w-xl rounded-[26px] border border-rose-200/30 bg-[#1a1020] p-6 shadow-2xl"><div className="flex items-center gap-3"><div className="grid h-12 w-12 place-items-center rounded-2xl bg-rose-300/15 text-rose-100"><Trash2 /></div><div><h4 className="text-lg font-black">Permanently erase quarantine?</h4><p className="text-xs text-slate-300">This removes every LumaSift quarantine file and cannot be undone.</p></div></div><label className="mt-5 block text-xs font-bold text-slate-200">Type <span className="font-mono text-rose-100">ERASE LUMASIFT QUARANTINE</span> to continue<input value={purgePhrase} onChange={(event) => setPurgePhrase(event.target.value)} className="cv-input mt-2 w-full font-mono" autoFocus /></label><div className="mt-5 flex justify-end gap-2"><button type="button" onClick={() => { setShowPurge(false); setPurgePhrase(""); }} disabled={applying} className="cv-btn cv-btn-secondary">Cancel</button><button type="button" onClick={() => void purgeQuarantine()} disabled={applying || purgePhrase !== "ERASE LUMASIFT QUARANTINE"} className="cv-btn cv-btn-danger disabled:opacity-50">{applying ? <LoaderCircle size={15} className="animate-spin" /> : <Trash2 size={15} />} Permanently erase</button></div></motion.div></motion.div>}</AnimatePresence>
    </div>
  );
}
