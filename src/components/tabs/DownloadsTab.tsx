// CinaVault Premium — Downloads Tab (yt-dlp + FFmpeg + MediaInfo + MKVToolNix)
import React, { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion } from "framer-motion";
import { useAppStore } from "../../store/appStore";
import {
  AlertTriangle,
  CheckCircle,
  Download,
  FileSearch,
  Loader,
  PackageSearch,
  Radio,
  Sparkles,
  Wrench,
  XCircle,
} from "lucide-react";

type ToolRecord = {
  id?: string;
  installed: boolean;
  version?: string | null;
};

type ToolStatusMap = Record<string, ToolRecord>;

export default function DownloadsTab() {
  const { downloading, setDownloading, addStatusMessage } = useAppStore();
  const [url, setUrl] = useState("");
  const [outputDir, setOutputDir] = useState("");
  const [isPlaylist, setIsPlaylist] = useState(false);
  const [toolStatus, setToolStatus] = useState<ToolStatusMap | null>(null);
  const [history, setHistory] = useState<any[]>([]);
  const [mediaPath, setMediaPath] = useState("");
  const [inspectionResult, setInspectionResult] = useState<unknown>(null);
  const [inspectingTool, setInspectingTool] = useState<
    "mediainfo" | "mkvtoolnix" | null
  >(null);

  const isHls = useMemo(() => /\.m3u8(?:$|\?)/i.test(url.trim()), [url]);

  const checkTools = async () => {
    try {
      const status = await invoke<{
        ready: boolean;
        tools: Array<{
          id: string;
          installed: boolean;
          version?: string | null;
        }>;
      }>("get_media_tools_status");
      setToolStatus(
        Object.fromEntries(
          status.tools.map((tool) => [tool.id.replace("-", "_"), tool]),
        ),
      );
    } catch {
      setToolStatus({
        yt_dlp: { installed: false },
        ffmpeg: { installed: false },
        ffprobe: { installed: false },
        mediainfo: { installed: false },
        mkvtoolnix: { installed: false },
      });
    }
  };

  useEffect(() => {
    void checkTools();
  }, []);

  const startDownload = async () => {
    if (!url) return;
    setDownloading(true);
    addStatusMessage(
      isHls
        ? `Starting HLS decode/download through yt-dlp + FFmpeg: ${url}`
        : `Starting download: ${url}`,
    );
    try {
      const result = await invoke<any>("ai_automated_download", {
        url,
        outputDir: outputDir || undefined,
        includePlaylist: isPlaylist,
        cookiesFile: undefined,
      });
      addStatusMessage(
        `${isHls ? "HLS stream" : "AI Download & Organize"} ${result.status}: ${result.title || url}`,
      );
      setHistory((prev) => [
        {
          url,
          status: result.status,
          title: result.title,
          kind: result.media_kind || (isHls ? "hls" : "media"),
          time: new Date().toLocaleTimeString(),
        },
        ...prev,
      ]);
    } catch (error) {
      addStatusMessage(`${isHls ? "HLS download" : "Download"} failed: ${error}`);
    } finally {
      setDownloading(false);
      setUrl("");
    }
  };

  const installTools = async () => {
    addStatusMessage(
      "Automatically checking and repairing permanent media tools...",
    );
    try {
      const result = await invoke<{
        ready: boolean;
        tools: Array<{ id: string; installed: boolean }>;
      }>("ensure_media_tools");
      const missing = result.tools
        .filter((tool) => !tool.installed)
        .map((tool) => tool.id);
      addStatusMessage(
        result.ready
          ? "All permanent media and download tools are loaded"
          : `Automatic setup could not load: ${missing.join(", ")}`,
      );
      await checkTools();
    } catch (error) {
      addStatusMessage(`Automatic tool repair failed: ${error}`);
    }
  };

  const inspectMedia = async (tool: "mediainfo" | "mkvtoolnix") => {
    const path = mediaPath.trim();
    if (!path) return;
    setInspectingTool(tool);
    setInspectionResult(null);
    try {
      const command =
        tool === "mediainfo"
          ? "inspect_with_mediainfo"
          : "inspect_with_mkvtoolnix";
      const result = await invoke<unknown>(command, { path });
      setInspectionResult(result);
      addStatusMessage(
        `${tool === "mediainfo" ? "MediaInfo" : "MKVToolNix"} inspection completed`,
      );
    } catch (error) {
      const message = String(error);
      setInspectionResult({ error: message });
      addStatusMessage(
        `${tool === "mediainfo" ? "MediaInfo" : "MKVToolNix"} inspection failed: ${message}`,
      );
    } finally {
      setInspectingTool(null);
    }
  };

  return (
    <div className="space-y-5">
      <div className="glass-panel p-5">
        <h3 className="text-sm font-bold mb-3 flex items-center gap-2">
          <Wrench size={16} className="text-cv-accent" /> Download Tools Status
        </h3>
        <div className="flex flex-wrap gap-4">
          {[
            { name: "yt-dlp", status: toolStatus?.yt_dlp },
            { name: "FFmpeg", status: toolStatus?.ffmpeg },
            { name: "FFprobe", status: toolStatus?.ffprobe },
            { name: "MediaInfo", status: toolStatus?.mediainfo },
            { name: "MKVToolNix", status: toolStatus?.mkvtoolnix },
          ].map((tool) => (
            <div
              key={tool.name}
              className="glass-panel-2 px-4 py-3 rounded-lg flex items-center gap-3"
              title={tool.status?.version || undefined}
            >
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
          <button
            type="button"
            onClick={() => void installTools()}
            className="cv-btn cv-btn-secondary self-center"
          >
            <Wrench size={14} /> Recheck & Repair Automatically
          </button>
        </div>
      </div>

      <div className="glass-panel p-5">
        <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
          <FileSearch size={16} className="text-cv-accent" /> Media File Tools
        </h3>
        <div className="space-y-3">
          <div>
            <label className="section-label">Media file path</label>
            <input
              type="text"
              value={mediaPath}
              onChange={(event) => setMediaPath(event.target.value)}
              placeholder="C:\\Media\\Movie.mkv"
              className="cv-input"
            />
          </div>
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              onClick={() => void inspectMedia("mediainfo")}
              disabled={
                !mediaPath.trim() ||
                !toolStatus?.mediainfo?.installed ||
                inspectingTool !== null
              }
              className="cv-btn cv-btn-primary"
            >
              {inspectingTool === "mediainfo" ? (
                <Loader size={14} className="animate-spin" />
              ) : (
                <FileSearch size={14} />
              )}
              Run MediaInfo
            </button>
            <button
              type="button"
              onClick={() => void inspectMedia("mkvtoolnix")}
              disabled={
                !mediaPath.trim() ||
                !toolStatus?.mkvtoolnix?.installed ||
                inspectingTool !== null
              }
              className="cv-btn cv-btn-secondary"
            >
              {inspectingTool === "mkvtoolnix" ? (
                <Loader size={14} className="animate-spin" />
              ) : (
                <PackageSearch size={14} />
              )}
              Run MKVToolNix
            </button>
          </div>
          {inspectionResult !== null && (
            <pre className="max-h-96 overflow-auto rounded-xl border border-white/10 bg-black/30 p-4 text-xs whitespace-pre-wrap break-words">
              {JSON.stringify(inspectionResult, null, 2)}
            </pre>
          )}
        </div>
      </div>

      <div className="glass-panel p-5">
        <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
          <Sparkles size={16} className="text-cv-accent" /> AI Automated Web Download
        </h3>
        <div className="space-y-3">
          <div>
            <label className="section-label">URL</label>
            <input
              type="text"
              value={url}
              onChange={(event) => setUrl(event.target.value)}
              placeholder="https://example.com/master.m3u8 or any supported media URL"
              className="cv-input"
            />
          </div>
          <div className="glass-panel-2 rounded-lg px-4 py-3 flex items-start gap-3">
            <Sparkles size={16} className="text-cv-accent mt-0.5" />
            <div>
              <div className="text-sm font-semibold">One-link automation enabled</div>
              <div className="text-[11px] text-cv-subtext">
                CinaVault validates the link, repairs required tools, downloads the best available format, detects completed files, verifies adult metadata when applicable, retrieves provider artwork, and writes NFO sidecars automatically.
              </div>
            </div>
          </div>
          {isHls && (
            <div className="glass-panel-2 rounded-lg px-4 py-3 flex items-start gap-3">
              <Radio size={16} className="text-cv-accent mt-0.5" />
              <div>
                <div className="text-sm font-semibold">HLS stream detected</div>
                <div className="text-[11px] text-cv-subtext">
                  CinaVault will hand the .m3u8 playlist to yt-dlp and FFmpeg, select the best available rendition, decode/remux the HLS segments, and save a normal media file when the source is not DRM-protected.
                </div>
              </div>
            </div>
          )}
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="section-label">
                Output Directory (optional)
              </label>
              <input
                type="text"
                value={outputDir}
                onChange={(event) => setOutputDir(event.target.value)}
                placeholder="Default: Downloads folder"
                className="cv-input"
              />
            </div>
            <div className="flex items-end gap-3">
              <label className="flex items-center gap-2 cursor-pointer">
                <div
                  className={`cv-toggle ${isPlaylist ? "active" : ""}`}
                  onClick={() => setIsPlaylist(!isPlaylist)}
                />
                <span className="text-sm">Playlist Mode</span>
              </label>
            </div>
          </div>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => void startDownload()}
              disabled={downloading || !url}
              className="cv-btn cv-btn-primary"
            >
              {downloading ? (
                <Loader size={14} className="animate-spin" />
              ) : isHls ? (
                <Radio size={14} />
              ) : (
                <Download size={14} />
              )}
              {downloading
                ? "Downloading..."
                : isPlaylist
                  ? "Download Playlist"
                  : isHls
                    ? "Decode & Download HLS"
                    : "Download"}
            </button>
            {downloading && (
              <button
                type="button"
                onClick={() => void invoke("cancel_download")}
                className="cv-btn cv-btn-danger"
              >
                Cancel
              </button>
            )}
          </div>
        </div>
      </div>

      <div className="glass-panel rounded-xl overflow-hidden">
        <div className="px-5 py-3 border-b border-white/5">
          <h3 className="text-sm font-bold">Download History</h3>
        </div>
        {history.length === 0 ? (
          <div className="p-8 text-center">
            <Download size={36} className="mx-auto text-cv-subtext/20 mb-3" />
            <p className="text-sm text-cv-subtext">
              No downloads yet. Paste a URL above to get started.
            </p>
          </div>
        ) : (
          <div className="divide-y divide-white/5 max-h-80 overflow-y-auto">
            {history.map((item, index) => (
              <motion.div
                key={`${item.url}-${item.time}-${index}`}
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                className="px-5 py-3 flex items-center gap-3"
              >
                {item.status === "completed" ? (
                  <CheckCircle size={16} className="text-green-500 shrink-0" />
                ) : (
                  <AlertTriangle
                    size={16}
                    className="text-cv-danger shrink-0"
                  />
                )}
                <div className="flex-1 min-w-0">
                  <div className="text-sm font-medium truncate">
                    {item.title || item.url}
                  </div>
                  <div className="text-[10px] text-cv-subtext truncate">
                    {item.kind === "hls" ? "HLS · " : ""}
                    {item.url}
                  </div>
                </div>
                <span className="text-[10px] text-cv-subtext shrink-0">
                  {item.time}
                </span>
              </motion.div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
