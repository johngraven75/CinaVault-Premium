import { useCallback, useEffect, useState } from "react";
import { ExternalLink, Loader, RefreshCw, Search, Sparkles } from "lucide-react";
import {
  fetchPublicHuggingFaceModels,
  type HuggingFaceModel,
} from "../../services/huggingFaceModelCatalog";

type Props = {
  activeModel: string;
  onUseModel: (modelId: string) => Promise<void> | void;
  onStatus?: (message: string) => void;
};

export default function HuggingFaceCatalogPanel({
  activeModel,
  onUseModel,
  onStatus,
}: Props) {
  const [search, setSearch] = useState("");
  const [task, setTask] = useState("text-generation");
  const [models, setModels] = useState<HuggingFaceModel[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [applying, setApplying] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await fetchPublicHuggingFaceModels({
        search: search.trim() || undefined,
        task,
        limit: 60,
      });
      setModels(result);
      onStatus?.(`Hugging Face catalog loaded: ${result.length} compatible models`);
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause);
      setError(message);
      onStatus?.(`Hugging Face catalog failed: ${message}`);
    } finally {
      setLoading(false);
    }
  }, [onStatus, search, task]);

  useEffect(() => {
    void load();
  }, [load]);

  const useModel = async (modelId: string) => {
    setApplying(modelId);
    try {
      await onUseModel(modelId);
      onStatus?.(`Hugging Face model selected: ${modelId}`);
    } finally {
      setApplying(null);
    }
  };

  return (
    <section className="glass-panel-2 rounded-xl p-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h4 className="flex items-center gap-2 text-sm font-bold">
            <Sparkles size={15} className="text-cv-accent" /> Hugging Face Model Catalog
          </h4>
          <p className="mt-1 text-[10px] text-cv-subtext">
            Browse public, ungated reasoning and text-generation models directly in the app.
          </p>
        </div>
        <button type="button" onClick={() => void load()} disabled={loading} className="cv-btn cv-btn-secondary text-xs disabled:opacity-50">
          <RefreshCw size={12} className={loading ? "animate-spin" : ""} /> Refresh
        </button>
      </div>

      <div className="mt-4 grid grid-cols-1 gap-3 md:grid-cols-[1fr_220px_auto]">
        <div className="relative">
          <Search size={14} className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-cv-subtext" />
          <input value={search} onChange={(event) => setSearch(event.target.value)} onKeyDown={(event) => event.key === "Enter" && void load()} placeholder="Search models, publishers, tags..." className="cv-input w-full pl-9" />
        </div>
        <select value={task} onChange={(event) => setTask(event.target.value)} className="cv-select w-full">
          <option value="text-generation">Text generation</option>
          <option value="image-text-to-text">Vision + text</option>
          <option value="text-classification">Text classification</option>
          <option value="feature-extraction">Embeddings</option>
        </select>
        <button type="button" onClick={() => void load()} disabled={loading} className="cv-btn cv-btn-primary disabled:opacity-50">
          {loading ? <Loader size={13} className="animate-spin" /> : <Search size={13} />} Search
        </button>
      </div>

      {error && (
        <div className="mt-4 rounded-lg border border-red-300/20 bg-red-400/10 p-3 text-xs text-red-100">
          <div className="font-semibold">Catalog unavailable</div>
          <div className="mt-1 break-words text-[10px] opacity-80">{error}</div>
          <div className="mt-2 text-[10px] opacity-70">Check internet access, Windows firewall, VPN state, and Hugging Face availability.</div>
        </div>
      )}

      <div className="mt-4 max-h-[420px] space-y-2 overflow-y-auto pr-1">
        {!loading && !error && models.length === 0 && (
          <div className="rounded-lg border border-white/10 bg-black/20 p-6 text-center text-xs text-cv-subtext">No compatible public models matched this search.</div>
        )}
        {models.map((item) => {
          const selected = activeModel === item.id || activeModel.startsWith(`${item.id}:`);
          return (
            <article key={item.id} className={`rounded-xl border p-3 ${selected ? "border-cv-accent/40 bg-cv-accent/10" : "border-white/10 bg-black/20"}`}>
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div className="min-w-0 flex-1">
                  <div className="truncate text-xs font-bold text-cv-text">{item.id}</div>
                  <div className="mt-1 flex flex-wrap gap-2 text-[9px] text-cv-subtext">
                    <span>{item.pipelineTag || "unknown task"}</span>
                    <span>{item.libraryName || "unknown library"}</span>
                    <span>{(item.downloads || 0).toLocaleString()} downloads</span>
                    <span>{(item.likes || 0).toLocaleString()} likes</span>
                  </div>
                  {item.tags?.length > 0 && <div className="mt-2 line-clamp-2 text-[9px] text-cv-subtext">{item.tags.slice(0, 10).join(" · ")}</div>}
                </div>
                <div className="flex shrink-0 gap-2">
                  <button type="button" onClick={() => window.open(`https://huggingface.co/${item.id}`, "_blank", "noopener,noreferrer")} className="cv-btn cv-btn-secondary px-2 py-1 text-[10px]" title="Open model page">
                    <ExternalLink size={11} /> View
                  </button>
                  <button type="button" onClick={() => void useModel(item.id)} disabled={selected || applying === item.id} className="cv-btn cv-btn-primary px-2 py-1 text-[10px] disabled:opacity-50">
                    {applying === item.id ? <Loader size={11} className="animate-spin" /> : <Sparkles size={11} />} {selected ? "Selected" : "Use Model"}
                  </button>
                </div>
              </div>
            </article>
          );
        })}
      </div>
    </section>
  );
}
