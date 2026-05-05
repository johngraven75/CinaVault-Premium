// CinaVault Premium — Downloads Tab (yt-dlp + ffmpeg)
import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion } from "framer-motion";
import { useAppStore } from "../../store/appStore";
import { Download, ListVideo, Wrench, CheckCircle, XCircle, Loader, Link, FolderOpen, AlertTriangle } from "lucide-react";

export default function DownloadsTab() {
  const { downloading, setDownloading, addStatusMessage } = useAppStore();
  const [url, setUrl] = useState("");
  const [outputDir, setOutputDir] = useState("");
  const [isPlaylist, setIsPlaylist] = useState(false);
  const [toolStatus, setToolStatus] = useState<any>(null);
  const [history, setHistory] = useState<any[]>([]);

  useEffect(() => { checkTools(); }, []);

  const checkTools = async () => {
    try {
      const status = await invoke<any>("check_download_tools");
      setToolStatus(status);
    } catch {
      setToolStatus({ yt_dlp: { installed: false }, ffmpeg: { installed: false }, ffprobe: { installed: false } });
    }
  };

  const startDownload = async () => {
    if (!url) return;
    setDownloading(true);
    addStatusMessage(`Starting download: ${url}`);
    try {
      const cmd = isPlaylist ? "start_playlist_download" : "start_download";
      const result = await invoke<any>(cmd, { url, outputDir: outputDir || undefined });
      addStatusMessage(`Download ${result.status}: ${result.title || url}`);
      setHistory(prev => [{ url, status: result.status, title: result.title, time: new Date().toLocaleTimeString() }, ...prev]);
    } catch (e) { addStatusMessage(`Download failed: ${e}`); }
    setDownloading(false);
    setUrl("");
  };

  const installTools = async () => {
    addStatusMessage("Installing download tools via winget...");
    try {
      const result = await invoke<any>("install_download_tools");
      addStatusMessage("Download tools installation complete");
      checkTools();
    } catch (e) { addStatusMessage(`Install failed: ${e}`); }
  };

  return (
    <div className="space-y-5">
      {/* Tool Status */}
      <div className="glass-panel p-5">
        <h3 className="text-sm font-bold mb-3 flex items-center gap-2">
          <Wrench size={16} className="text-cv-accent" /> Download Tools Status
        </h3>
        <div className="flex flex-wrap gap-4">
          {[
            { name: "yt-dlp", status: toolStatus?.yt_dlp },
            { name: "FFmpeg", status: toolStatus?.ffmpeg },
            { name: "FFprobe", status: toolStatus?.ffprobe },
          ].map(tool => (
            <div key={tool.name} className="glass-panel-2 px-4 py-3 rounded-lg flex items-center gap-3">
              {tool.status?.installed ? (
                <CheckCircle size={16} className="text-green-500" />
              ) : (
                <XCircle size={16} className="text-cv-danger" />
              )}
              <div>
                <div className="text-sm font-semibold">{tool.name}</div>
                <div className="text-[10px] text-cv-subtext">
                  {tool.status?.installed ? "Installed" : "Not Found"}
                </div>
              </div>
            </div>
          ))}
          <button onClick={installTools} className="cv-btn cv-btn-secondary self-center">
            <Wrench size={14} /> Install via winget
          </button>
        </div>
      </div>

      {/* Download Panel */}
      <div className="glass-panel p-5">
        <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
          <Download size={16} className="text-cv-accent" /> Web / URL Download
        </h3>
        <div className="space-y-3">
          <div>
            <label className="section-label">URL</label>
            <input
              type="text" value={url} onChange={e => setUrl(e.target.value)}
              placeholder="https://youtube.com/watch?v=... or any supported URL"
              className="cv-input"
            />
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="section-label">Output Directory (optional)</label>
              <input
                type="text" value={outputDir} onChange={e => setOutputDir(e.target.value)}
                placeholder="Default: Downloads folder"
                className="cv-input"
              />
            </div>
            <div className="flex items-end gap-3">
              <label className="flex items-center gap-2 cursor-pointer">
                <div className={`cv-toggle ${isPlaylist ? "active" : ""}`} onClick={() => setIsPlaylist(!isPlaylist)} />
                <span className="text-sm">Playlist Mode</span>
              </label>
            </div>
          </div>
          <div className="flex gap-2">
            <button onClick={startDownload} disabled={downloading || !url} className="cv-btn cv-btn-primary">
              {downloading ? <Loader size={14} className="animate-spin" /> : <Download size={14} />}
              {downloading ? "Downloading..." : isPlaylist ? "Download Playlist" : "Download"}
            </button>
            {downloading && (
              <button onClick={() => invoke("cancel_download")} className="cv-btn cv-btn-danger">
                Cancel
              </button>
            )}
          </div>
        </div>
      </div>

      {/* Download History */}
      <div className="glass-panel rounded-xl overflow-hidden">
        <div className="px-5 py-3 border-b border-white/5">
          <h3 className="text-sm font-bold">Download History</h3>
        </div>
        {history.length === 0 ? (
          <div className="p-8 text-center">
            <Download size={36} className="mx-auto text-cv-subtext/20 mb-3" />
            <p className="text-sm text-cv-subtext">No downloads yet. Paste a URL above to get started.</p>
          </div>
        ) : (
          <div className="divide-y divide-white/5 max-h-80 overflow-y-auto">
            {history.map((item, i) => (
              <motion.div
                key={i}
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                className="px-5 py-3 flex items-center gap-3"
              >
                {item.status === "completed" ? (
                  <CheckCircle size={16} className="text-green-500 shrink-0" />
                ) : (
                  <AlertTriangle size={16} className="text-cv-danger shrink-0" />
                )}
                <div className="flex-1 min-w-0">
                  <div className="text-sm font-medium truncate">{item.title || item.url}</div>
                  <div className="text-[10px] text-cv-subtext truncate">{item.url}</div>
                </div>
                <span className="text-[10px] text-cv-subtext shrink-0">{item.time}</span>
              </motion.div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
