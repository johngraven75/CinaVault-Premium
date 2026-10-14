// CinaVault Premium — embedded Live TV player.
//
// HLS playlists play through hls.js, raw MPEG-TS (Xtream /live/…/id.ts)
// through mpegts.js, and plain files through the <video> element. Every
// stream goes through the loopback relay (iptv_relay.rs) because IPTV servers
// rarely send CORS headers. Both libraries load on first use and run without
// blob workers, which the app's CSP does not allow. A stream's User-Agent /
// Referer (from the playlist or the source's settings) is registered with the
// relay, which sends it upstream for the playlist and every segment.
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import {
  AlertTriangle,
  Loader2,
  Maximize,
  Minimize,
  Pause,
  PictureInPicture2,
  Play,
  RotateCcw,
  Settings,
  Volume2,
  VolumeX,
} from "lucide-react";
import { detectStreamKind, type StreamKind } from "../utils/iptvStream";
import {
  DEFAULT_PLAYER_SETTINGS,
  hlsConfigFor,
  mpegtsConfigFor,
  pickHlsLevel,
  type PlayerSettings,
} from "../utils/iptvPlayerSettings";

type Status = "loading" | "playing" | "paused" | "error";

interface Engine {
  destroy: () => void;
}

const STREAM_KIND_LABELS: Record<StreamKind, string> = { hls: "HLS", mpegts: "MPEG-TS", native: "Direct" };

interface IPTVPlayerProps {
  streamUrl: string;
  title?: string;
  subtitle?: string | null;
  /** Bumped by the parent to force a reconnect (e.g. channel re-selected). */
  reloadKey?: number;
  /** Sent upstream by the relay for this stream. */
  userAgent?: string | null;
  referrer?: string | null;
  settings?: PlayerSettings;
  /** Shows the gear button. */
  onOpenSettings?: () => void;
  /** Rendered inside the player so it stays visible in full screen. */
  settingsOverlay?: ReactNode;
}

const VOLUME_KEY = "cinavault.livetv.volume";

function readVolume(): number {
  try {
    const raw = localStorage.getItem(VOLUME_KEY);
    const stored = raw === null ? Number.NaN : Number(raw);
    return stored >= 0 && stored <= 1 ? stored : 0.8;
  } catch {
    return 0.8;
  }
}

async function relayUrl(url: string, userAgent: string | null, referrer: string | null): Promise<string> {
  // Outside the desktop shell (vite dev in a browser) play directly.
  if (!isTauri()) return url;
  // In the app every stream goes through the relay: most IPTV servers send no
  // CORS headers, so a direct fallback would only fail later with a vaguer error.
  try {
    return await invoke<string>("get_stream_relay_url", { url, userAgent, referrer });
  } catch (reason) {
    throw new Error(`The stream relay could not start: ${reason instanceof Error ? reason.message : String(reason)}`);
  }
}

async function attachHls(
  video: HTMLVideoElement,
  src: string,
  settings: PlayerSettings,
  onFatal: (message: string) => void,
): Promise<Engine> {
  const { default: Hls } = await import("hls.js");
  if (!Hls.isSupported()) {
    if (video.canPlayType("application/vnd.apple.mpegurl")) {
      video.src = src;
      return { destroy: () => video.removeAttribute("src") };
    }
    throw new Error("This system cannot play HLS streams.");
  }
  const hls = new Hls(hlsConfigFor(settings));
  let networkRetries = 0;
  let mediaRecoveries = 0;
  hls.on(Hls.Events.MANIFEST_PARSED, (_event, data) => {
    networkRetries = 0;
    const level = pickHlsLevel(settings.quality, data.levels.length);
    if (level >= 0) hls.currentLevel = level;
  });
  hls.on(Hls.Events.ERROR, (_event, data) => {
    if (!data.fatal) return;
    if (data.type === Hls.ErrorTypes.NETWORK_ERROR && networkRetries < settings.reconnectAttempts) {
      networkRetries += 1;
      window.setTimeout(() => hls.startLoad(), 1000 * networkRetries);
    } else if (data.type === Hls.ErrorTypes.MEDIA_ERROR && mediaRecoveries < 2) {
      mediaRecoveries += 1;
      hls.recoverMediaError();
    } else {
      onFatal(
        data.type === Hls.ErrorTypes.NETWORK_ERROR
          ? "The channel is not responding. It may be offline or your subscription may have expired."
          : "This stream uses a format the player cannot decode.",
      );
    }
  });
  hls.loadSource(src);
  hls.attachMedia(video);
  return { destroy: () => hls.destroy() };
}

async function attachMpegts(
  video: HTMLVideoElement,
  src: string,
  settings: PlayerSettings,
  onFatal: (message: string) => void,
): Promise<Engine> {
  const { default: mpegts } = await import("mpegts.js");
  if (!mpegts.getFeatureList().mseLivePlayback) {
    video.src = src;
    return { destroy: () => video.removeAttribute("src") };
  }
  const player = mpegts.createPlayer(
    { type: "mpegts", isLive: true, url: src },
    mpegtsConfigFor(settings),
  );
  // Live TS endpoints drop connections often; reconnect a few times before giving up.
  let networkRetries = 0;
  let retryTimer: number | undefined;
  const onPlaying = () => {
    networkRetries = 0;
  };
  video.addEventListener("playing", onPlaying);
  player.on(mpegts.Events.ERROR, (type: string) => {
    if (type === mpegts.ErrorTypes.NETWORK_ERROR && networkRetries < settings.reconnectAttempts) {
      networkRetries += 1;
      retryTimer = window.setTimeout(() => {
        player.unload();
        player.load();
        void Promise.resolve(player.play()).catch(() => undefined);
      }, 1000 * networkRetries);
      return;
    }
    onFatal(
      type === mpegts.ErrorTypes.NETWORK_ERROR
        ? "The channel is not responding. It may be offline or your subscription may have expired."
        : "This stream uses a format the player cannot decode.",
    );
  });
  player.attachMediaElement(video);
  player.load();
  return {
    destroy: () => {
      window.clearTimeout(retryTimer);
      video.removeEventListener("playing", onPlaying);
      try {
        player.pause();
        player.unload();
        player.detachMediaElement();
      } finally {
        player.destroy();
      }
    },
  };
}

export default function IPTVPlayer({
  streamUrl,
  title,
  subtitle,
  reloadKey = 0,
  userAgent = null,
  referrer = null,
  settings = DEFAULT_PLAYER_SETTINGS,
  onOpenSettings,
  settingsOverlay,
}: Readonly<IPTVPlayerProps>) {
  const shellRef = useRef<HTMLDivElement>(null);
  const videoRef = useRef<HTMLVideoElement>(null);
  const engineRef = useRef<Engine | null>(null);
  const hideTimer = useRef<number | undefined>(undefined);
  const [status, setStatus] = useState<Status>("loading");
  const [error, setError] = useState<string | null>(null);
  const [kind, setKind] = useState<StreamKind>(() => detectStreamKind(streamUrl));
  const [volume, setVolume] = useState(readVolume);
  const [muted, setMuted] = useState(false);
  const [fullscreen, setFullscreen] = useState(false);
  const [controlsVisible, setControlsVisible] = useState(true);
  const [attempt, setAttempt] = useState(0);
  // Engine options are read when a stream (re)connects, not on every change,
  // so adjusting a slider does not drop the picture.
  const settingsRef = useRef(settings);
  settingsRef.current = settings;
  const { latency, bufferSeconds, quality, reconnectAttempts } = settings;

  // (Re)connect whenever the channel, a retry or the parent asks for it.
  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    if (!streamUrl) {
      setError("This channel has no stream address.");
      setStatus("error");
      return;
    }
    let cancelled = false;
    const streamKind = detectStreamKind(streamUrl);
    setKind(streamKind);
    setStatus("loading");
    setError(null);

    const fail = (message: string) => {
      if (cancelled) return;
      setError(message);
      setStatus("error");
    };

    void (async () => {
      const current = settingsRef.current;
      try {
        const src = await relayUrl(streamUrl, userAgent, referrer);
        if (cancelled) return;
        let engine: Engine;
        if (streamKind === "hls") engine = await attachHls(video, src, current, fail);
        else if (streamKind === "mpegts") engine = await attachMpegts(video, src, current, fail);
        else {
          video.src = src;
          engine = { destroy: () => video.removeAttribute("src") };
        }
        if (cancelled) {
          engine.destroy();
          return;
        }
        engineRef.current = engine;
        if (!current.autoplay) {
          setStatus("paused");
          return;
        }
        await video.play().catch(() => {
          // Autoplay with sound can be refused; the play button still works.
          if (!cancelled) setStatus("paused");
        });
      } catch (reason) {
        fail(reason instanceof Error ? reason.message : String(reason));
      }
    })();

    return () => {
      cancelled = true;
      engineRef.current?.destroy();
      engineRef.current = null;
      video.removeAttribute("src");
      video.load();
    };
    // Engine settings reconnect the stream so the new buffer / quality applies.
  }, [streamUrl, attempt, reloadKey, userAgent, referrer, latency, bufferSeconds, quality, reconnectAttempts]);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    const onPlaying = () => setStatus("playing");
    const onPause = () => setStatus((current) => (current === "error" ? current : "paused"));
    const onWaiting = () => setStatus((current) => (current === "error" ? current : "loading"));
    const onError = () => {
      if (kind === "native") {
        setError("The stream could not be opened.");
        setStatus("error");
      }
    };
    video.addEventListener("playing", onPlaying);
    video.addEventListener("pause", onPause);
    video.addEventListener("waiting", onWaiting);
    video.addEventListener("error", onError);
    return () => {
      video.removeEventListener("playing", onPlaying);
      video.removeEventListener("pause", onPause);
      video.removeEventListener("waiting", onWaiting);
      video.removeEventListener("error", onError);
    };
  }, [kind]);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    video.volume = volume;
    video.muted = muted;
    try {
      localStorage.setItem(VOLUME_KEY, String(volume));
    } catch {
      // Storage can be unavailable; the volume still applies.
    }
  }, [volume, muted]);

  useEffect(() => {
    const onChange = () => setFullscreen(document.fullscreenElement === shellRef.current);
    document.addEventListener("fullscreenchange", onChange);
    return () => document.removeEventListener("fullscreenchange", onChange);
  }, []);

  useEffect(() => () => window.clearTimeout(hideTimer.current), []);

  const revealControls = useCallback(() => {
    setControlsVisible(true);
    window.clearTimeout(hideTimer.current);
    hideTimer.current = window.setTimeout(() => setControlsVisible(false), 3000);
  }, []);

  const togglePlay = useCallback(() => {
    const video = videoRef.current;
    if (!video) return;
    if (video.paused) {
      // Jump back to the live edge after a pause.
      if (video.buffered.length) {
        const liveEdge = video.buffered.end(video.buffered.length - 1);
        if (liveEdge - video.currentTime > 10) video.currentTime = liveEdge - 2;
      }
      void video.play().catch(() => undefined);
    } else {
      video.pause();
    }
  }, []);

  const toggleFullscreen = useCallback(() => {
    const shell = shellRef.current;
    if (!shell) return;
    if (document.fullscreenElement) void document.exitFullscreen();
    else void shell.requestFullscreen().catch(() => undefined);
  }, []);

  const togglePip = useCallback(() => {
    const video = videoRef.current;
    if (!video || !document.pictureInPictureEnabled) return;
    if (document.pictureInPictureElement) void document.exitPictureInPicture();
    else void video.requestPictureInPicture().catch(() => undefined);
  }, []);

  const playButtonRef = useRef<HTMLButtonElement>(null);

  // Clicking the picture plays or pauses and moves focus into the controls so
  // the keyboard shortcuts below work straight away.
  const onVideoClick = () => {
    togglePlay();
    playButtonRef.current?.focus({ preventScroll: true });
  };

  // Shortcuts while any player control has focus (keydown bubbles to the shell).
  const onKeyDown = useCallback((event: KeyboardEvent) => {
    const target = event.target;
    if (
      target instanceof HTMLInputElement ||
      target instanceof HTMLSelectElement ||
      target instanceof HTMLTextAreaElement ||
      (target instanceof Element && target.closest("[data-player-settings]"))
    ) {
      return;
    }
    if (event.key === " " || event.key === "k") {
      event.preventDefault();
      togglePlay();
    } else if (event.key === "f") {
      toggleFullscreen();
    } else if (event.key === "m") {
      setMuted((value) => !value);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setVolume((value) => Math.min(1, Math.round((value + 0.05) * 100) / 100));
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      setVolume((value) => Math.max(0, Math.round((value - 0.05) * 100) / 100));
    }
    revealControls();
  }, [togglePlay, toggleFullscreen, revealControls]);

  useEffect(() => {
    const shell = shellRef.current;
    if (!shell) return;
    shell.addEventListener("keydown", onKeyDown);
    return () => shell.removeEventListener("keydown", onKeyDown);
  }, [onKeyDown]);

  const showControls = controlsVisible || status !== "playing";

  return (
    <div
      ref={shellRef}
      role="region"
      onMouseMove={revealControls}
      onMouseLeave={() => status === "playing" && setControlsVisible(false)}
      className={`group relative w-full overflow-hidden rounded-xl bg-black outline-none ring-cv-accent/60 focus-visible:ring-2 ${fullscreen ? "" : "aspect-video"}`}
      style={{ cursor: showControls ? "default" : "none" }}
      aria-label={title ? `Live player: ${title}` : "Live player"}
    >
      <video
        ref={videoRef}
        className="h-full w-full bg-black"
        style={{ objectFit: settings.videoFit }}
        playsInline
        onClick={onVideoClick}
        onDoubleClick={toggleFullscreen}
      >
        {/* Live channels ship no caption files; the empty track keeps the element accessible-ready. */}
        <track kind="captions" />
      </video>

      {status === "loading" && !error && (
        <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center gap-2 text-white/80">
          <Loader2 size={34} className="animate-spin text-cv-accent" />
          <span className="text-xs tracking-wide">Tuning in…</span>
        </div>
      )}

      {error && (
        <div className="absolute inset-0 flex flex-col items-center justify-center gap-3 bg-black/75 px-6 text-center">
          <AlertTriangle size={30} className="text-amber-400" />
          <p className="max-w-md text-sm text-white/90">{error}</p>
          <button onClick={() => setAttempt((value) => value + 1)} className="cv-btn cv-btn-primary text-xs">
            <RotateCcw size={12} /> Try again
          </button>
        </div>
      )}

      {/* Top bar: channel and what's on */}
      <div
        className={`pointer-events-none absolute inset-x-0 top-0 flex items-start justify-between gap-3 bg-gradient-to-b from-black/80 to-transparent px-4 pb-8 pt-3 transition-opacity duration-300 ${showControls ? "opacity-100" : "opacity-0"}`}
      >
        <div className="min-w-0">
          {title && <div className="truncate text-sm font-bold text-white">{title}</div>}
          {subtitle && <div className="truncate text-xs text-white/70">{subtitle}</div>}
        </div>
        <span className="flex shrink-0 items-center gap-1.5 rounded-full bg-red-600/90 px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider text-white">
          <span className={`h-1.5 w-1.5 rounded-full bg-white ${status === "playing" ? "animate-pulse" : ""}`} />
          <span>Live</span>
        </span>
      </div>

      {/* Bottom controls */}
      <div
        className={`absolute inset-x-0 bottom-0 flex items-center gap-2 bg-gradient-to-t from-black/85 to-transparent px-3 pb-3 pt-10 transition-opacity duration-300 ${showControls ? "opacity-100" : "opacity-0"}`}
      >
        <button ref={playButtonRef} onClick={togglePlay} className="rounded-md p-2 text-white hover:bg-white/10" aria-label={status === "playing" ? "Pause" : "Play"}>
          {status === "playing" ? <Pause size={18} /> : <Play size={18} />}
        </button>
        <button onClick={() => setMuted((value) => !value)} className="rounded-md p-2 text-white hover:bg-white/10" aria-label={muted ? "Unmute" : "Mute"}>
          {muted || volume === 0 ? <VolumeX size={18} /> : <Volume2 size={18} />}
        </button>
        <input
          type="range"
          min={0}
          max={1}
          step={0.01}
          value={muted ? 0 : volume}
          onChange={(event) => {
            setVolume(Number(event.target.value));
            setMuted(false);
          }}
          className="w-24 accent-[var(--cv-accent)]"
          aria-label="Volume"
        />
        <span className="ml-1 text-[10px] uppercase tracking-wider text-white/50">
          {STREAM_KIND_LABELS[kind]}
        </span>
        <div className="flex-1" />
        <button onClick={() => setAttempt((value) => value + 1)} className="rounded-md p-2 text-white hover:bg-white/10" aria-label="Reconnect" title="Reconnect">
          <RotateCcw size={16} />
        </button>
        {onOpenSettings && (
          <button onClick={onOpenSettings} className="rounded-md p-2 text-white hover:bg-white/10" aria-label="Player settings" title="Settings">
            <Settings size={16} />
          </button>
        )}
        {typeof document !== "undefined" && document.pictureInPictureEnabled && (
          <button onClick={togglePip} className="rounded-md p-2 text-white hover:bg-white/10" aria-label="Picture in picture" title="Picture in picture">
            <PictureInPicture2 size={16} />
          </button>
        )}
        <button onClick={toggleFullscreen} className="rounded-md p-2 text-white hover:bg-white/10" aria-label={fullscreen ? "Exit full screen" : "Full screen"}>
          {fullscreen ? <Minimize size={16} /> : <Maximize size={16} />}
        </button>
      </div>

      {settingsOverlay}
    </div>
  );
}
