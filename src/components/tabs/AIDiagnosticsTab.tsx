// CinaVault Premium — AI Diagnostics Tab
import React, { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion } from "framer-motion";
import { useAppStore } from "../../store/appStore";
import AIVisualizer from "../effects/AIVisualizer";
import {
  formatMetadataTaskProgress,
  metadataTaskPopupVisible,
  MetadataTaskProgress,
} from "../../utils/metadataTaskProgress";
import { Brain, Send, Settings, Key, Cpu, Network, FolderSearch, Database, Loader, Sparkles, ExternalLink, Tag } from "lucide-react";

export default function AIDiagnosticsTab() {
  const { aiProcessing, setAiProcessing, aiResult, setAiResult, addStatusMessage } = useAppStore();
  const [prompt, setPrompt] = useState("");
  const [imageUrl, setImageUrl] = useState("");
  const [hfToken, setHfToken] = useState("");
  const [model, setModel] = useState("katanemo/Arch-Router-1.5B:hf-inference");
  const [showConfig, setShowConfig] = useState(false);
  const [history, setHistory] = useState<{ query: string; result: any; time: string }[]>([]);
  const [metadataProgress, setMetadataProgress] = useState<MetadataTaskProgress | null>(null);

  useEffect(() => {
    if (!aiProcessing) return;

    let cancelled = false;
    const pollProgress = async () => {
      try {
        const progress = await invoke<MetadataTaskProgress>("get_metadata_task_progress");
        if (!cancelled && metadataTaskPopupVisible(progress)) {
          setMetadataProgress(progress);
        }
      } catch {}
    };

    pollProgress();
    const timer = window.setInterval(pollProgress, 500);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [aiProcessing]);

  const formattedProgress = useMemo(
    () => formatMetadataTaskProgress(metadataProgress, "Metadata Task"),
    [metadataProgress],
  );
  const showMetadataProgress = metadataTaskPopupVisible(metadataProgress);

  const showStartingProgress = (label: string, task = "metadata_task") => {
    setMetadataProgress({
      active: true,
      task,
      label,
      current: 0,
      total: 1,
      percent: 0,
      message: `Starting ${label}...`,
    });
  };

  const showFinishedProgress = (label: string, message = `${label} complete`) => {
    setMetadataProgress(prev => ({
      ...prev,
      active: false,
      task: prev?.task || "metadata_task",
      label: prev?.label || label,
      current: prev?.total || 1,
      total: prev?.total || 1,
      percent: 100,
      message,
    }));
    window.setTimeout(() => {
      setMetadataProgress(current => current?.active ? current : null);
    }, 3500);
  };

  const runQuery = async () => {
    if (!prompt.trim()) return;
    const tracksAdultGather = /adult metadata|gather metadata|chapter images|adult providers/i.test(prompt);
    if (tracksAdultGather) {
      showStartingProgress("Adult Metadata Gather", "adult_metadata_gather");
    }
    setAiProcessing(true);
    addStatusMessage(`AI processing: ${prompt.substring(0, 50)}...`);
    try {
      const result = await invoke<any>("ai_query", { prompt });
      setAiResult(result);
      setHistory(prev => [{ query: prompt, result, time: new Date().toLocaleTimeString() }, ...prev.slice(0, 19)]);
      addStatusMessage("AI query complete");
      if (tracksAdultGather) {
        showFinishedProgress("Adult Metadata Gather", "Adult metadata gather complete");
      }
    } catch (e) {
      const errResult = { status: "error", message: String(e) };
      setAiResult(errResult);
      setHistory(prev => [{ query: prompt, result: errResult, time: new Date().toLocaleTimeString() }, ...prev.slice(0, 19)]);
      addStatusMessage(`AI error: ${e}`);
      if (tracksAdultGather) {
        showFinishedProgress("Adult Metadata Gather", `Adult metadata gather failed: ${e}`);
      }
    }
    setAiProcessing(false);
    setPrompt("");
  };

  const runInference = async () => {
    if (!prompt.trim()) return;
    setAiProcessing(true);
    try {
      const result = await invoke<any>("ai_inference", { input: prompt, model, imageUrl: imageUrl.trim() || null });
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

  const openLink = (url: string) => {
    window.open(url, "_blank", "noopener,noreferrer");
  };

  const runQuickAction = async (action: { label: string; q: string; progressTask?: string; runNow?: () => Promise<any> }) => {
    if (aiProcessing) return;
    setPrompt(action.q);
    if (!action.runNow) return;

    if (action.progressTask) {
      showStartingProgress(action.label, action.progressTask);
    }
    setAiProcessing(true);
    addStatusMessage(`Running: ${action.label}...`);
    try {
      const result = await action.runNow();
      setAiResult(result);
      setHistory(prev => [{ query: action.q, result, time: new Date().toLocaleTimeString() }, ...prev.slice(0, 19)]);
      if (result?.type === "library_enrichment") {
        addStatusMessage(`${action.label}: ${result.metadata_updated || 0} metadata updates, ${result.files_renamed || 0} files renamed`);
      } else {
        addStatusMessage(`${action.label} complete`);
      }
      if (action.progressTask) {
        showFinishedProgress(action.label);
      }
    } catch (e) {
      addStatusMessage(`${action.label} failed: ${e}`);
      if (action.progressTask) {
        showFinishedProgress(action.label, `${action.label} failed: ${e}`);
      }
    } finally {
      setAiProcessing(false);
    }
  };

  const quickActions: { label: string; icon: any; q: string; progressTask?: string; runNow?: () => Promise<any> }[] = [
    { label: "Network Diagnostics", icon: Network, q: "Run network diagnostics" },
    { label: "Check Sources", icon: FolderSearch, q: "Check all media sources" },
    { label: "Check Providers", icon: Database, q: "Check metadata providers" },
    {
      label: "Enrich Metadata",
      icon: Database,
      q: "Enrich Library Metadata",
      progressTask: "library_enrichment",
      runNow: () => invoke("run_library_enrichment", { renameFiles: false }),
    },
    {
      label: "Normalize Filenames",
      icon: Tag,
      q: "Enrich + Normalize Filenames",
      progressTask: "library_enrichment",
      runNow: () => invoke("run_library_enrichment", { renameFiles: true }),
    },
    {
      label: "Adult Metadata Gather",
      icon: Sparkles,
      q: "Run adult metadata gather for installed providers and generate posters and chapter images",
      progressTask: "adult_metadata_gather",
      runNow: () => invoke("ai_query", { prompt: "Run adult metadata gather for installed providers and generate posters and chapter images" }),
    },
  ];

  return (
    <div className="space-y-5">
      {showMetadataProgress && (
        <motion.div
          initial={{ opacity: 0, y: 16, scale: 0.98 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={{ opacity: 0, y: 16, scale: 0.98 }}
          className="fixed right-5 bottom-5 z-[90] w-[min(360px,calc(100vw-2rem))] glass-panel p-4 border border-cv-accent/25 shadow-2xl"
        >
          <div className="flex items-start justify-between gap-4">
            <div className="min-w-0">
              <div className="text-sm font-bold text-cv-text truncate">{formattedProgress.label}</div>
              <div className="text-[11px] text-cv-subtext mt-1 truncate">{formattedProgress.message}</div>
            </div>
            <div className="text-xl font-bold text-cv-accent tabular-nums">{formattedProgress.percent}%</div>
          </div>
          <div className="mt-3 h-2 rounded-full bg-white/10 overflow-hidden">
            <motion.div
              className="h-full rounded-full"
              style={{ background: "linear-gradient(90deg, var(--cv-accent), var(--cv-neon-1))" }}
              animate={{ width: `${formattedProgress.percent}%` }}
              transition={{ duration: 0.25 }}
            />
          </div>
          {formattedProgress.total > 0 && (
            <div className="mt-2 text-[10px] text-cv-subtext tabular-nums">
              {formattedProgress.current} / {formattedProgress.total} items
            </div>
          )}
        </motion.div>
      )}

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
          <div className="mt-2">
            <input
              type="text"
              value={imageUrl}
              onChange={e => setImageUrl(e.target.value)}
              placeholder="Optional image URL for multimodal query (jpg/png/webp)"
              className="cv-input w-full bg-black/30 text-xs"
            />
          </div>

          {/* Quick Actions */}
          <div className="flex gap-2 mt-3">
            {quickActions.map(action => (
              <button
                key={action.label}
                disabled={aiProcessing}
                onClick={() => runQuickAction(action)}
                className="cv-btn cv-btn-secondary text-[10px] py-1 disabled:opacity-50 disabled:cursor-not-allowed"
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
                <button onClick={() => openLink("https://huggingface.co/settings/tokens")} className="cv-btn cv-btn-secondary text-xs">
                  <ExternalLink size={12} /> Get API Key
                </button>
              </div>
              <div className="text-[10px] text-cv-subtext mt-1">Get a token from huggingface.co/settings/tokens</div>
            </div>
            <div>
              <label className="section-label">AI Model</label>
              <div className="flex gap-2">
                <input value={model} onChange={e => setModel(e.target.value)} className="cv-input flex-1" placeholder="katanemo/Arch-Router-1.5B:hf-inference" />
                <button onClick={saveModel} className="cv-btn cv-btn-primary text-xs"><Cpu size={12} /> Set</button>
              </div>
              <div className="text-[10px] text-cv-subtext mt-1">Default: katanemo/Arch-Router-1.5B:hf-inference</div>
            </div>
          </div>
          <div className="mt-3 text-[10px] text-cv-subtext">
            Inference URL: https://router.huggingface.co/v1/chat/completions
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
