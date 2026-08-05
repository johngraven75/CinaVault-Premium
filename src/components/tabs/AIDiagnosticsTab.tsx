import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Brain, Cpu, Key, Loader, Send, Settings, ShieldCheck, Sparkles } from "lucide-react";
import { useAppStore } from "../../store/appStore";
import HuggingFaceCatalogPanel from "../ai/HuggingFaceCatalogPanel";

const DEFAULT_HF_MODEL = "katanemo/Arch-Router-1.5B:hf-inference";

type AiConfig = {
  model?: string;
  default_model?: string;
  has_token?: boolean;
  inference_url?: string;
};

export default function AIDiagnosticsTab() {
  const { aiProcessing, setAiProcessing, aiResult, setAiResult, addStatusMessage } = useAppStore();
  const [prompt, setPrompt] = useState("");
  const [model, setModel] = useState(DEFAULT_HF_MODEL);
  const [hfToken, setHfToken] = useState("");
  const [hasHfToken, setHasHfToken] = useState(false);
  const [showConfig, setShowConfig] = useState(true);

  const loadConfig = async () => {
    try {
      await invoke("ensure_hf_token");
      const config = await invoke<AiConfig>("get_ai_config");
      setModel(config.model || config.default_model || DEFAULT_HF_MODEL);
      setHasHfToken(Boolean(config.has_token));
    } catch (error) {
      addStatusMessage(`AI configuration unavailable: ${error}`);
    }
  };

  useEffect(() => { void loadConfig(); }, []);

  const saveToken = async () => {
    try {
      await invoke("set_hf_token", { token: hfToken.trim() });
      setHfToken("");
      await loadConfig();
      addStatusMessage("Hugging Face token saved");
    } catch (error) {
      addStatusMessage(`Hugging Face token save failed: ${error}`);
    }
  };

  const useModel = async (modelId: string) => {
    await invoke("set_ai_model", { model: modelId });
    setModel(modelId);
    addStatusMessage(`AI model selected: ${modelId}`);
  };

  const runQuery = async () => {
    const value = prompt.trim();
    if (!value || aiProcessing) return;
    setAiProcessing(true);
    try {
      const result = await invoke("ai_query", { prompt: value });
      setAiResult(result);
      addStatusMessage("AI query complete");
    } catch (error) {
      setAiResult({ status: "error", message: String(error) });
      addStatusMessage(`AI query failed: ${error}`);
    } finally {
      setAiProcessing(false);
    }
  };

  return (
    <div className="space-y-5">
      <section className="glass-panel p-5">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <h3 className="flex items-center gap-2 text-sm font-bold"><Brain size={16} className="text-cv-accent" /> AI Autopilot</h3>
            <p className="mt-1 text-xs text-cv-subtext">Run AI tasks and choose Hugging Face models directly from the user interface.</p>
          </div>
          <button type="button" onClick={() => setShowConfig((value) => !value)} className="cv-btn cv-btn-secondary text-xs"><Settings size={12} /> {showConfig ? "Hide configuration" : "Configure"}</button>
        </div>

        <div className="mt-4 flex gap-2">
          <input value={prompt} onChange={(event) => setPrompt(event.target.value)} onKeyDown={(event) => event.key === "Enter" && void runQuery()} className="cv-input flex-1" placeholder="Ask AI to scan, identify, enrich, repair, or organize media" />
          <button type="button" onClick={() => void runQuery()} disabled={aiProcessing || !prompt.trim()} className="cv-btn cv-btn-primary disabled:opacity-50">
            {aiProcessing ? <Loader size={13} className="animate-spin" /> : <Send size={13} />} Run
          </button>
        </div>

        <div className="mt-3 grid grid-cols-1 gap-3 md:grid-cols-2">
          <div className="glass-panel-2 rounded-xl p-3"><div className="text-[10px] uppercase tracking-wider text-cv-subtext">Current model</div><div className="mt-1 truncate text-xs font-bold">{model}</div></div>
          <div className="glass-panel-2 rounded-xl p-3"><div className="text-[10px] uppercase tracking-wider text-cv-subtext">Hugging Face token</div><div className="mt-1 flex items-center gap-2 text-xs font-bold"><ShieldCheck size={13} className={hasHfToken ? "text-emerald-300" : "text-amber-300"} />{hasHfToken ? "Configured" : "Missing"}</div></div>
        </div>
      </section>

      {showConfig && (
        <section className="glass-panel p-5 space-y-4">
          <div>
            <h3 className="flex items-center gap-2 text-sm font-bold"><Cpu size={16} className="text-cv-accent" /> AI Configuration</h3>
            <div className="mt-3 flex gap-2">
              <input type="password" value={hfToken} onChange={(event) => setHfToken(event.target.value)} placeholder={hasHfToken ? "Token already configured" : "hf_..."} className="cv-input flex-1" />
              <button type="button" onClick={() => void saveToken()} disabled={!hfToken.trim()} className="cv-btn cv-btn-primary text-xs disabled:opacity-50"><Key size={12} /> Save token</button>
            </div>
          </div>
          <HuggingFaceCatalogPanel activeModel={model} onUseModel={useModel} onStatus={addStatusMessage} />
        </section>
      )}

      {aiResult && <section className="glass-panel p-5"><h3 className="mb-3 flex items-center gap-2 text-sm font-bold"><Sparkles size={15} className="text-cv-accent" /> Latest AI result</h3><pre className="max-h-80 overflow-auto whitespace-pre-wrap rounded-xl bg-black/25 p-4 text-xs text-cv-subtext">{JSON.stringify(aiResult, null, 2)}</pre></section>}
    </div>
  );
}
