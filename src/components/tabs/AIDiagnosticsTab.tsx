// CinaVault Premium — AI Diagnostics Tab
import React, { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion } from "framer-motion";
import { useAppStore } from "../../store/appStore";
import AIVisualizer from "../effects/AIVisualizer";
import { Brain, Send, Settings, Key, Cpu, Network, FolderSearch, Database, Loader, Sparkles } from "lucide-react";

export default function AIDiagnosticsTab() {
  const { aiProcessing, setAiProcessing, aiResult, setAiResult, addStatusMessage } = useAppStore();
  const [prompt, setPrompt] = useState("");
  const [hfToken, setHfToken] = useState("");
  const [model, setModel] = useState("facebook/bart-large-cnn");
  const [showConfig, setShowConfig] = useState(false);
  const [history, setHistory] = useState<{ query: string; result: any; time: string }[]>([]);

  const runQuery = async () => {
    if (!prompt.trim()) return;
    setAiProcessing(true);
    addStatusMessage(`AI processing: ${prompt.substring(0, 50)}...`);
    try {
      const result = await invoke<any>("ai_query", { prompt });
      setAiResult(result);
      setHistory(prev => [{ query: prompt, result, time: new Date().toLocaleTimeString() }, ...prev.slice(0, 19)]);
      addStatusMessage("AI query complete");
    } catch (e) {
      const errResult = { status: "error", message: String(e) };
      setAiResult(errResult);
      setHistory(prev => [{ query: prompt, result: errResult, time: new Date().toLocaleTimeString() }, ...prev.slice(0, 19)]);
      addStatusMessage(`AI error: ${e}`);
    }
    setAiProcessing(false);
    setPrompt("");
  };

  const runInference = async () => {
    if (!prompt.trim()) return;
    setAiProcessing(true);
    try {
      const result = await invoke<any>("ai_inference", { input: prompt, model });
      setAiResult(result);
      setHistory(prev => [{ query: `[Inference] ${prompt}`, result, time: new Date().toLocaleTimeString() }, ...prev.slice(0, 19)]);
      addStatusMessage("AI inference complete");
    } catch (e) { addStatusMessage(`Inference failed: ${e}`); }
    setAiProcessing(false);
  };

  const saveToken = async () => {
    try {
      await invoke("set_hf_token", { token: hfToken });
      addStatusMessage("HuggingFace token saved");
    } catch (e) { addStatusMessage(`Failed: ${e}`); }
  };

  const saveModel = async () => {
    try {
      await invoke("set_ai_model", { model });
      addStatusMessage(`AI model set to: ${model}`);
    } catch (e) { addStatusMessage(`Failed: ${e}`); }
  };

  return (
    <div className="space-y-5">
      {/* AI Visualizer */}
      <div className="glass-panel p-5 relative overflow-hidden" style={{ minHeight: 280 }}>
        <div className="absolute inset-0 z-0">
          <AIVisualizer active={aiProcessing} />
        </div>
        <div className="relative z-10">
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-sm font-bold flex items-center gap-2">
              <Brain size={16} className="text-cv-accent" /> AI Agent
            </h3>
            <div className="flex gap-2">
              <button onClick={() => setShowConfig(!showConfig)} className="cv-btn cv-btn-secondary text-xs">
                <Settings size={12} /> Configure
              </button>
            </div>
          </div>

          {/* Prompt Input */}
          <div className="flex gap-3 mt-auto pt-32">
            <input
              type="text"
              value={prompt}
              onChange={e => setPrompt(e.target.value)}
              onKeyDown={e => e.key === "Enter" && runQuery()}
              placeholder="Ask AI anything... (network diagnostics, source checks, provider checks, or general inference)"
              className="cv-input flex-1 bg-black/40"
            />
            <button onClick={runQuery} disabled={aiProcessing} className="cv-btn cv-btn-primary">
              {aiProcessing ? <Loader size={14} className="animate-spin" /> : <Send size={14} />}
              Query
            </button>
            <button onClick={runInference} disabled={aiProcessing} className="cv-btn cv-btn-gold">
              <Sparkles size={14} /> Inference
            </button>
          </div>

          {/* Quick Actions */}
          <div className="flex gap-2 mt-3">
            {[
              { label: "Network Diagnostics", icon: Network, q: "Run network diagnostics" },
              { label: "Check Sources", icon: FolderSearch, q: "Check all media sources" },
              { label: "Check Providers", icon: Database, q: "Check metadata providers" },
            ].map(action => (
              <button
                key={action.label}
                onClick={() => { setPrompt(action.q); }}
                className="cv-btn cv-btn-secondary text-[10px] py-1"
              >
                <action.icon size={10} /> {action.label}
              </button>
            ))}
          </div>
        </div>
      </div>

      {/* AI Configuration */}
      {showConfig && (
        <motion.div initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} className="glass-panel p-5">
          <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
            <Cpu size={16} className="text-cv-accent" /> AI Model Configuration
          </h3>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div>
              <label className="section-label">HuggingFace Token</label>
              <div className="flex gap-2">
                <input type="password" value={hfToken} onChange={e => setHfToken(e.target.value)} className="cv-input flex-1" placeholder="hf_..." />
                <button onClick={saveToken} className="cv-btn cv-btn-primary text-xs"><Key size={12} /> Save</button>
              </div>
              <div className="text-[10px] text-cv-subtext mt-1">Get a token from huggingface.co/settings/tokens</div>
            </div>
            <div>
              <label className="section-label">AI Model</label>
              <div className="flex gap-2">
                <input value={model} onChange={e => setModel(e.target.value)} className="cv-input flex-1" placeholder="facebook/bart-large-cnn" />
                <button onClick={saveModel} className="cv-btn cv-btn-primary text-xs"><Cpu size={12} /> Set</button>
              </div>
              <div className="text-[10px] text-cv-subtext mt-1">Default: facebook/bart-large-cnn via HF Inference Router</div>
            </div>
          </div>
          <div className="mt-3 text-[10px] text-cv-subtext">
            Inference URL: https://router.huggingface.co/hf-inference/models
          </div>
        </motion.div>
      )}

      {/* Results & History */}
      {(aiResult || history.length > 0) && (
        <div className="glass-panel p-5">
          <h3 className="text-sm font-bold mb-3">AI Activity Log</h3>

          {aiResult && (
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              className="glass-panel-2 p-4 rounded-lg mb-3"
            >
              <div className="text-xs font-semibold mb-2 text-cv-accent">Latest Result</div>
              <pre className="text-xs text-cv-subtext whitespace-pre-wrap font-mono max-h-48 overflow-y-auto">
                {JSON.stringify(aiResult, null, 2)}
              </pre>
            </motion.div>
          )}

          {history.length > 0 && (
            <div className="space-y-1 max-h-60 overflow-y-auto">
              {history.map((entry, i) => (
                <div key={i} className="flex items-start gap-3 py-2 px-3 rounded hover:bg-white/[0.02] text-xs">
                  <span className="text-cv-subtext shrink-0">{entry.time}</span>
                  <div className="flex-1 min-w-0">
                    <div className="font-medium truncate">{entry.query}</div>
                    <div className="text-cv-subtext text-[10px] truncate">
                      {entry.result?.status || "completed"} — {entry.result?.type || "inference"}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
