// CinaVault Premium — Build 132 Cinematic Command Header
import { useRef, useEffect, useState } from "react";
import type { JSX } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { useAppStore, TabId } from "../store/appStore";
import { Search, Bell, Maximize2, Activity, RadioTower, Sparkles } from "lucide-react";
import { getUnreadStatusMessages } from "../utils/pluginUiSafety";

const TAB_LABELS: Record<TabId, string> = {
  home: "Library",
  sources: "Media Sources",
  downloads: "Downloads",
  livetv: "Live TV",
  server: "Server",
  security: "Security",
  remote: "Remote Access",
  advanced: "Advanced",
  cloud: "Cloud & NAS",
  plugins: "Plugins & Metadata",
  ai: "AI Diagnostics",
  settings: "Settings",
};

const TAB_SUBTITLES: Record<TabId, string> = {
  home: "Browse, curate, and launch your media universe",
  sources: "Ingest folders, drives, libraries, and scan targets",
  downloads: "Monitor queues, transfers, and acquisition jobs",
  livetv: "Tune streams, guide data, and channel intelligence",
  server: "Core services, uptime, networking, and health telemetry",
  security: "Access control, hardening, privacy, and audit state",
  remote: "Relay status, external reachability, and secure access",
  advanced: "Expert controls, tuning knobs, and platform diagnostics",
  cloud: "Cloud, NAS, sync, and storage fabric management",
  plugins: "Metadata, compatibility layers, and extension control",
  ai: "Predictive diagnostics and intelligent repair suggestions",
  settings: "Preferences, profile, themes, and app behavior",
};

interface Particle {
  x: number;
  y: number;
  z: number;
  size: number;
  drift: number;
}

interface Comet {
  x: number;
  y: number;
  speed: number;
  length: number;
  angle: number;
  life: number;
  maxLife: number;
}

export default function Header(): JSX.Element {
  const { activeTab, searchQuery, setSearchQuery, statusMessages } = useAppStore();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const mouseRef = useRef({ x: 0, y: 0 });
  const [clock, setClock] = useState(new Date());
  const [showNotifications, setShowNotifications] = useState(false);
  const [lastReadMessageIndex, setLastReadMessageIndex] = useState(0);
  const unreadMessages = getUnreadStatusMessages(statusMessages, lastReadMessageIndex);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const context = canvas.getContext("2d");
    if (!context) return;

    let animationFrame = 0;
    let width = 0;
    let height = 0;
    let particles: Particle[] = [];
    let comets: Comet[] = [];

    const resize = (): void => {
      const ratio = Math.max(1, window.devicePixelRatio || 1);
      width = canvas.offsetWidth;
      height = canvas.offsetHeight;
      canvas.width = Math.max(1, Math.floor(width * ratio));
      canvas.height = Math.max(1, Math.floor(height * ratio));
      context.setTransform(ratio, 0, 0, ratio, 0, 0);
      particles = Array.from({ length: 130 }, () => ({
        x: Math.random() * width,
        y: Math.random() * height,
        z: Math.random(),
        size: Math.random() * 1.4 + 0.35,
        drift: Math.random() * 0.18 + 0.03,
      }));
    };

    const spawnComet = (): void => {
      if (comets.length < 2 && Math.random() < 0.006) {
        comets.push({
          x: -48,
          y: Math.random() * height * 0.62,
          speed: 2.1 + Math.random() * 2.8,
          length: 56 + Math.random() * 80,
          angle: -0.11 + Math.random() * 0.22,
          life: 0,
          maxLife: 126 + Math.random() * 90,
        });
      }
    };

    const draw = (): void => {
      context.clearRect(0, 0, width, height);

      const nebula = context.createRadialGradient(width * 0.74, height * 0.24, 0, width * 0.74, height * 0.24, width * 0.7);
      nebula.addColorStop(0, "rgba(0,234,255,0.105)");
      nebula.addColorStop(0.45, "rgba(255,77,184,0.045)");
      nebula.addColorStop(1, "transparent");
      context.fillStyle = nebula;
      context.fillRect(0, 0, width, height);

      const mx = (mouseRef.current.x - width / 2) * 0.028;
      const my = (mouseRef.current.y - height / 2) * 0.028;
      const now = Date.now();

      for (const particle of particles) {
        particle.x += particle.drift;
        if (particle.x > width + 4) particle.x = -4;

        const px = particle.x + mx * particle.z;
        const py = particle.y + my * particle.z;
        const alpha = 0.24 + particle.z * 0.54 + Math.sin(now * 0.002 + particle.x) * 0.12;

        context.beginPath();
        context.arc(px, py, particle.size, 0, Math.PI * 2);
        context.fillStyle = `rgba(223,251,255,${alpha})`;
        context.fill();
      }

      spawnComet();
      comets = comets.filter((comet) => comet.life < comet.maxLife);
      for (const comet of comets) {
        comet.x += comet.speed * Math.cos(comet.angle);
        comet.y += comet.speed * Math.sin(comet.angle);
        comet.life += 1;

        const fadeIn = Math.min(comet.life / 18, 1);
        const fadeOut = Math.max(1 - (comet.life - comet.maxLife + 28) / 28, 0);
        const alpha = fadeIn * fadeOut * 0.72;
        const tail = context.createLinearGradient(
          comet.x,
          comet.y,
          comet.x - comet.length * Math.cos(comet.angle),
          comet.y - comet.length * Math.sin(comet.angle),
        );

        tail.addColorStop(0, `rgba(255,255,255,${alpha})`);
        tail.addColorStop(0.38, `rgba(0,234,255,${alpha * 0.55})`);
        tail.addColorStop(1, "transparent");
        context.beginPath();
        context.moveTo(comet.x, comet.y);
        context.lineTo(comet.x - comet.length * Math.cos(comet.angle), comet.y - comet.length * Math.sin(comet.angle));
        context.strokeStyle = tail;
        context.lineWidth = 1.6;
        context.stroke();
      }

      animationFrame = requestAnimationFrame(draw);
    };

    const handleMouseMove = (event: MouseEvent): void => {
      const rect = canvas.getBoundingClientRect();
      mouseRef.current = { x: event.clientX - rect.left, y: event.clientY - rect.top };
    };

    resize();
    draw();
    window.addEventListener("resize", resize);
    canvas.addEventListener("mousemove", handleMouseMove);

    return () => {
      cancelAnimationFrame(animationFrame);
      window.removeEventListener("resize", resize);
      canvas.removeEventListener("mousemove", handleMouseMove);
    };
  }, []);

  useEffect(() => {
    const timer = window.setInterval(() => setClock(new Date()), 1000);
    return () => window.clearInterval(timer);
  }, []);

  const toggleFullscreen = async (): Promise<void> => {
    try {
      if (document.fullscreenElement) {
        await document.exitFullscreen();
      } else {
        await document.documentElement.requestFullscreen();
      }
    } catch {}
  };

  return (
    <header className="cv-header relative z-20 h-[88px] shrink-0 overflow-visible border-b border-white/10 bg-black/10">
      <canvas ref={canvasRef} className="absolute inset-0 h-full w-full" style={{ pointerEvents: "auto" }} />
      <div className="pointer-events-none absolute inset-0 bg-[linear-gradient(90deg,rgba(0,0,0,0.36),transparent_42%,rgba(0,234,255,0.08))]" />
      <div className="pointer-events-none absolute inset-x-6 bottom-0 h-px bg-gradient-to-r from-transparent via-white/30 to-transparent" />

      <div className="relative z-10 flex h-full items-center justify-between gap-5 px-5">
        <div className="flex min-w-0 items-center gap-4">
          <motion.div
            className="relative grid h-12 w-12 shrink-0 place-items-center rounded-2xl border border-white/15 bg-white/[0.055] shadow-[0_0_34px_rgba(0,234,255,0.16)]"
            whileHover={{ scale: 1.04 }}
          >
            <div className="absolute inset-1 rounded-xl bg-[radial-gradient(circle_at_30%_15%,rgba(255,255,255,0.26),transparent_44%)]" />
            <img src="/branding/cinavault-premium-mark.png" alt="CinaVault brand" className="relative h-9 w-9 rounded-xl object-cover" />
          </motion.div>

          <div className="min-w-0">
            <div className="mb-1 flex items-center gap-2 text-[10px] font-semibold uppercase tracking-[0.28em]" style={{ color: "var(--cv-accent)" }}>
              <Sparkles size={12} />
              Build 132 Interface
            </div>
            <AnimatePresence mode="wait">
              <motion.div
                key={activeTab}
                initial={{ opacity: 0, y: 10, filter: "blur(8px)" }}
                animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
                exit={{ opacity: 0, y: -8, filter: "blur(8px)" }}
                transition={{ duration: 0.24 }}
              >
                <h1 className="truncate text-xl font-black tracking-tight" style={{ color: "var(--cv-text)" }}>
                  {TAB_LABELS[activeTab]}
                </h1>
                <p className="truncate text-xs" style={{ color: "var(--cv-subtext)" }}>
                  {TAB_SUBTITLES[activeTab]}
                </p>
              </motion.div>
            </AnimatePresence>
          </div>
        </div>

        <div className="flex shrink-0 items-center gap-3">
          <div className="hidden items-center gap-2 rounded-xl border border-white/10 bg-white/[0.045] px-3 py-2 text-[11px] uppercase tracking-[0.18em] text-cv-subtext shadow-[inset_0_1px_0_rgba(255,255,255,0.08)] lg:flex">
            <Activity size={13} className="text-emerald-300" />
            Core Stable
          </div>

          <div className="relative cv-command-bar">
            <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-cv-subtext" />
            <input
              type="text"
              value={searchQuery}
              onChange={(event) => setSearchQuery(event.target.value)}
              placeholder="Search library, plugins, sources..."
              className="cv-input w-64 rounded-xl border-white/10 bg-black/24 pl-9 text-xs shadow-[inset_0_1px_0_rgba(255,255,255,0.08)]"
            />
          </div>

          <div className="hidden min-w-[86px] text-right font-mono text-[11px] tracking-wide text-cv-subtext/85 sm:block">
            {clock.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" })}
          </div>

          <button
            type="button"
            onClick={toggleFullscreen}
            className="grid h-10 w-10 place-items-center rounded-xl border border-white/10 bg-white/[0.045] text-cv-subtext hover:bg-white/[0.085] hover:text-cv-text"
            title="Toggle fullscreen"
          >
            <Maximize2 size={15} />
          </button>

          <button
            type="button"
            onClick={() => {
              setShowNotifications((open) => !open);
              setLastReadMessageIndex(Math.max(0, statusMessages.length - 1));
            }}
            className="relative grid h-10 w-10 place-items-center rounded-xl border border-white/10 bg-white/[0.045] text-cv-subtext hover:bg-white/[0.085] hover:text-cv-text"
            title="Show notifications"
          >
            <Bell size={15} />
            {unreadMessages.length > 0 && (
              <span className="absolute right-2 top-2 h-2 w-2 rounded-full bg-cyan-300 shadow-[0_0_14px_rgba(34,211,238,0.9)]" />
            )}
          </button>
        </div>
      </div>

      <AnimatePresence>
        {showNotifications && (
          <motion.div
            initial={{ opacity: 0, y: -8, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -8, scale: 0.98 }}
            transition={{ duration: 0.18 }}
            className="absolute right-5 top-[calc(100%+10px)] z-50 w-96 max-w-[calc(100vw-2rem)] overflow-hidden rounded-2xl border border-white/10 bg-[#07101b]/95 shadow-[0_26px_80px_rgba(0,0,0,0.48)] backdrop-blur-2xl"
          >
            <div className="flex items-center justify-between border-b border-white/10 px-4 py-3">
              <span className="flex items-center gap-2 text-xs font-bold" style={{ color: "var(--cv-text)" }}>
                <RadioTower size={14} style={{ color: "var(--cv-accent)" }} />
                Command Feed
              </span>
              <span className="text-[10px] uppercase tracking-[0.18em]" style={{ color: "var(--cv-subtext)" }}>
                {statusMessages.length} messages
              </span>
            </div>
            <div className="max-h-80 overflow-y-auto py-1">
              {statusMessages.length === 0 ? (
                <div className="px-4 py-5 text-xs" style={{ color: "var(--cv-subtext)" }}>No notifications yet</div>
              ) : (
                statusMessages.slice(-12).reverse().map((message, index) => (
                  <div key={`${message}-${index}`} className="border-b border-white/[0.05] px-4 py-3 last:border-b-0">
                    <div className="text-xs leading-relaxed" style={{ color: "var(--cv-text)" }}>{message}</div>
                  </div>
                ))
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </header>
  );
}
