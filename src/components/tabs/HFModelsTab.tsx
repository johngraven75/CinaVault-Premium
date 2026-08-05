import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Brain, CheckCircle, Key, Search, Sparkles } from "lucide-react";
import { useAppStore } from "../../store/appStore";

const MODELS = [
  { id: "Qwen/Qwen3-4B-Instruct-2507", name: "Qwen3 4B Instruct", reasoning: true, size: "4B", use: "Library automation" },
  { id: "HuggingFaceTB/SmolLM3-3B", name: "SmolLM3 3B", reasoning: true, size: "3B", use: "Fast local assistance" },
  { id: "deepseek-ai/DeepSeek-R1-Distill-Qwen-7B", name: "DeepSeek R1 Distill", reasoning: true, size: "7B", use: "Deeper planning" },
  { id: "microsoft/Phi-3.5-mini-instruct", name: "Phi 3.5 Mini", reasoning: false, size: "3.8B", use: "Efficient instructions" },
  { id: "katanemo/Arch-Router-1.5B:hf-inference", name: "Arch Router", reasoning: true, size: "1.5B", use: "Tool routing" },
] as const;

export default function HFModelsTab() {
  const addStatusMessage = useAppStore((state) => state.addStatusMessage);
  const [selected, setSelected] = useState(MODELS[0].id as string);
  const [saved, setSaved] = useState("");
  const [query, setQuery] = useState("");
  const [reasoningOnly, setReasoningOnly] = useState(true);
  const [hasToken, setHasToken] = useState(false);

  useEffect(() => {
    void invoke<any>("get_ai_config").then((config) => {
      setSaved(config.model || "");
      setSelected(config.model || MODELS[0].id);
      setHasToken(Boolean(config.has_token));
    });
  }, []);

  const visible = useMemo(() => MODELS.filter((model) =>
    (!reasoningOnly || model.reasoning) && `${model.name} ${model.id} ${model.use}`.toLowerCase().includes(query.toLowerCase())), [query, reasoningOnly]);

  const save = async () => {
    await invoke("set_ai_model", { model: selected });
    setSaved(selected);
    addStatusMessage(`Hugging Face model selected: ${selected}`);
  };

  return <div className="space-y-5">
    <section className="glass-panel p-5">
      <h3 className="flex items-center gap-2 text-lg font-bold"><Brain className="text-cv-accent" /> Hugging Face Models</h3>
      <p className="mt-2 text-xs text-cv-subtext">Choose from public, ungated models. Locked and gated models are excluded.</p>
      <div className="mt-4 grid gap-3 md:grid-cols-3">
        <div className="glass-panel-2 rounded-xl p-3"><div className="text-[10px] text-cv-subtext">CURRENT MODEL</div><div className="mt-1 break-all text-xs font-bold">{saved || "Not selected"}</div></div>
        <div className="glass-panel-2 rounded-xl p-3"><div className="text-[10px] text-cv-subtext">HF TOKEN</div><div className="mt-1 flex items-center gap-2 text-xs font-bold"><Key size={13} />{hasToken ? "Configured" : "Missing"}</div></div>
        <div className="glass-panel-2 rounded-xl p-3"><div className="text-[10px] text-cv-subtext">CATALOG POLICY</div><div className="mt-1 text-xs font-bold">Free · Public · Ungated</div></div>
      </div>
      <div className="mt-4 flex flex-wrap gap-3"><label className="cv-input flex min-w-64 flex-1 items-center gap-2"><Search size={14} /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search models or uses" className="w-full bg-transparent outline-none" /></label><label className="flex items-center gap-2 text-xs"><input type="checkbox" checked={reasoningOnly} onChange={(event) => setReasoningOnly(event.target.checked)} /> Reasoning models only</label></div>
    </section>
    <section className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">{visible.map((model) => <button key={model.id} onClick={() => setSelected(model.id)} className={`glass-panel p-4 text-left ${selected === model.id ? "border border-cv-accent" : "border border-transparent"}`}><div className="flex justify-between gap-2"><span className="font-bold">{model.name}</span>{model.reasoning && <span className="rounded bg-cv-accent/15 px-2 py-1 text-[9px] text-cv-accent">REASONING</span>}</div><div className="mt-2 break-all font-mono text-[10px] text-cv-subtext">{model.id}</div><div className="mt-3 flex justify-between text-xs text-cv-subtext"><span>{model.use}</span><span>{model.size}</span></div>{saved === model.id && <div className="mt-3 flex items-center gap-1 text-xs text-emerald-300"><CheckCircle size={12} /> Active</div>}</button>)}</section>
    <button onClick={() => void save()} disabled={!selected} className="cv-btn cv-btn-gold"><Sparkles size={14} /> Save and Use Selected Model</button>
  </div>;
}
