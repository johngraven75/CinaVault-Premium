// CinaVault Premium — Build 137 Cyber HUD Command Header
import { useEffect, useRef, useState } from "react";
import type { JSX } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Activity, Bell, Cpu, Maximize2, RadioTower, Search, ShieldCheck, Sparkles, Zap } from "lucide-react";
import { useAppStore, TabId } from "../store/appStore";
import { getUnreadStatusMessages } from "../utils/pluginUiSafety";

interface TabMeta {
  label: string;
  subtitle: string;
  signal: string;
}

const TAB_META: Record<TabId, TabMeta> = {
  home: {
    label: "Movies",
    subtitle: "Holographic library carousel, vault inventory, and instant playback",
    signal: "Vault",
  },
  sources: {
    label: "Media Sources",
    subtitle: "Ingest folders, drives, network shares, and scan targets",
    signal: "Ingest",
  },
  downloads: {
    label: "My Vault",
    subtitle: "Queue telemetry, acquisitions, and personal watch staging",
    signal: "Queue",
  },
  livetv: {
    label: "TV Shows",
    subtitle: "Live streams, channel intelligence, and guide signals",
    signal: "Stream",
  },
  server: {
    label: "System Core",
    subtitle: "Services, runtime health, networking, and uptime telemetry",
    signal: "Core",
  },
  security: {
    label: "Security",
    subtitle: "Access control, privacy shields, audit trails, and hardening",
    signal: "Guard",
  },
  remote: {
    label: "Remote",
    subtitle: "Relay state, external reachability, and secure remote paths",
    signal: "Relay",
  },
  advanced: {
    label: "Advanced",
    subtitle: "Expert tuning, debug controls, and platform diagnostics",
    signal: "Tune",
  },
  cloud: {
    label: "Cloud / NAS",
    subtitle: "Cloud sync, NAS fabric, and storage mesh control",
    signal: "Mesh",
  },
  plugins: {
    label: "Plugins",
    subtitle: "Metadata engines, compatibility bridges, and extension control",
    signal: "Mods",
  },
  ai: {
    label: "AI Terminal",
    subtitle: "Predictive diagnostics, repair guidance, and neural analysis",
    signal: "Neural",
  },
  settings: {
    label: "Settings",
    subtitle: "Themes, preferences, profiles, and cinematic behavior",
    signal: "Config",
  },
};

const PRIMARY_TABS: TabId[] = ["home", "livetv", "downloads", "sources", "server", "plugins", "ai", "settings"];

interface Particle {
  x: number;
  y: number;
  vx: number;
  phase: number;
  size: number;
}

export default function Header(): JSX.Element {
  const { activeTab, setActiveTab, searchQuery, setSearchQuery, statusMessages } = useAppStore();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [clock, setClock] = useState(new Date());
  const [showNotifications, setShowNotifications] = useState(false);
  const [lastReadMessageIndex, setLastReadMessageIndex] = useState(0);
  const unreadMessages = getUnreadStatusMessages(statusMessages, lastReadMessageIndex);
  const activeMeta = TAB_META[activeTab] ?? TAB_META.home;

  useEffect(() => {
    const canvas = canvasRef.current;
    const context = canvas?.getContext("2d");
    if (!canvas || !context) return;

    let animationFrame = 0;
    let width = 0;
    let height = 0;
    let particles: Particle[] = [];

    const resize = (): void => {
      const ratio = Math.max(1, window.devicePixelRatio || 1);
      width = canvas.offsetWidth;
      height = canvas.offsetHeight;
      canvas.width = Math.max(1, Math.floor(width * ratio));
      canvas.height = Math.max(1, Math.floor(height * ratio));
      context.setTransform(ratio, 0, 0, ratio, 0, 0);
      particles = Array.from({ length: 96 }, () => ({
        x: Math.random() * width,
        y: Math.random() * height,
        vx: 0.18 + Math.random() * 0.72,
        phase: Math.random() * Math.PI * 2,
        size: 0.55 + Math.random() * 1.4,
      }));
    };

    const draw = (): void => {
      const now = performance.now();
      context.clearRect(0, 0, width, height);

      const glow = context.createRadialGradient(width * 0.18, height * 0.32, 0, width * 0.18, height * 0.32, width * 0.7);
      glow.addColorStop(0, "rgba(0,245,255,0.16)");
      glow.addColorStop(0.48, "rgba(189,0,255,0.055)");
      glow.addColorStop(1, "transparent");
      context.fillStyle = glow;
      context.fillRect(0, 0, width, height);

      context.globalCompositeOperation = "lighter";
      for (const particle of particles) {
        particle.x += particle.vx;
        if (particle.x > width + 8) particle.x = -8;
        const y = particle.y + Math.sin(now * 0.0015 + particle.phase) * 5;
        const alpha = 0.28 + Math.sin(now * 0.002 + particle.phase) * 0.16;
        context.beginPath();
        context.arc(particle.x, y, particle.size, 0, Math.PI * 2);
        context.fillStyle = `rgba(0,245,255,${Math.max(0.08, alpha)})`;
        context.fill();
      }

      const scanX = ((now * 0.08) % (width + 180)) - 180;
      const beam = context.createLinearGradient(scanX, 0, scanX + 180, 0);
      beam.addColorStop(0, "transparent");
      beam.addColorStop(0.48, "rgba(0,245,255,0.0)");
      beam.addColorStop(0.5, "rgba(0,245,255,0.36)");
      beam.addColorStop(1, "transparent");
      context.fillStyle = beam;
      context.fillRect(0, 0, width, height);
      context.globalCompositeOperation = "source-over";

      animationFrame = requestAnimationFrame(draw);
    };

    resize();
    draw();
    window.addEventListener("resize", resize);

    return () => {
      cancelAnimationFrame(animationFrame);
      window.removeEventListener("resize", resize);
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
    <header className="cyber-header relative z-20 shrink-0 overflow-visible">
      <canvas ref={canvasRef} className="absolute inset-0 h-full w-full" aria-hidden="true" />

      <div className="relative z-10 flex h-full flex-col gap-3 px-4 py-3 xl:px-5">
        <div className="flex min-h-0 items-center justify-between gap-4">
          <div className="flex min-w-0 items-center gap-3">
            <motion.div
              className="cyber-brand-chip h-12 px-3"
              whileHover={{ scale: 1.03 }}
              transition={{ type: "spring", stiffness: 360, damping: 24 }}
            >
              <Zap size={18} className="text-cyan-200" />
              <div className="min-w-0">
                <div className="text-[10px] font-black uppercase tracking-[0.28em] text-cyan-200">CinaVault B137</div>
                <div className="truncate text-sm font-black uppercase tracking-[0.16em]">Hyper-Neon Fusion</div>
              </div>
            </motion.div>

            <AnimatePresence mode="wait">
              <motion.div
                key={activeTab}
                initial={{ opacity: 0, y: 8, filter: "blur(8px)" }}
                animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
                exit={{ opacity: 0, y: -8, filter: "blur(8px)" }}
                transition={{ duration: 0.22 }}
                className="hidden min-w-0 md:block"
              >
                <div className="cyber-eyebrow flex items-center gap-2">
                  <Sparkles size={12} /> Quantum Grid Active / {activeMeta.signal}
                </div>
                <h1 className="cyber-title truncate text-xl font-black tracking-tight">{activeMeta.label}</h1>
                <p className="truncate text-xs text-cv-subtext">{activeMeta.subtitle}</p>
              </motion.div>
            </AnimatePresence>
          </div>

          <div className="flex shrink-0 items-center gap-2 lg:gap-3">
            <div className="hidden items-center gap-2 rounded-none border border-cyan-300/20 bg-black/40 px-3 py-2 text-[10px] font-black uppercase tracking-[0.18em] text-cyan-200 shadow-[0_0_18px_rgba(0,245,255,0.12)] lg:flex">
              <Activity size={13} className="text-emerald-300" />
              Nominal
            </div>

            <div className="cyber-search-core hidden sm:block">
              <Search size={15} className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-cyan-200" />
              <input
                type="text"
                value={searchQuery}
                onChange={(event) => setSearchQuery(event.target.value)}
                placeholder="AI search core..."
                className="cyber-search-input"
                aria-label="Search library, plugins, and sources"
              />
            </div>

            <div className="hidden min-w-[94px] text-right font-mono text-[11px] tracking-wide text-cv-subtext/90 md:block">
              {clock.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" })}
            </div>

            <button type="button" onClick={toggleFullscreen} className="cyber-button h-11 w-11 px-0" title="Toggle fullscreen">
              <Maximize2 size={15} />
            </button>

            <button
              type="button"
              onClick={() => {
                setShowNotifications((open) => !open);
                setLastReadMessageIndex(Math.max(0, statusMessages.length - 1));
              }}
              className="cyber-button relative h-11 w-11 px-0"
              title="Show command feed"
            >
              <Bell size={15} />
              {unreadMessages.length > 0 && (
                <span className="absolute right-2 top-2 h-2 w-2 rounded-full bg-[var(--cyber-amber)] shadow-[0_0_14px_rgba(255,153,0,0.95)]" />
              )}
            </button>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <nav className="quantum-nav flex-1" aria-label="Primary CinaVault navigation">
            {PRIMARY_TABS.map((tab) => {
              const meta = TAB_META[tab];
              const isActive = activeTab === tab;
              return (
                <button
                  key={tab}
                  type="button"
                  data-label={meta.label}
                  className={`quantum-tab ${isActive ? "is-active" : ""}`}
                  onClick={() => setActiveTab(tab)}
                >
                  {meta.label}
                </button>
              );
            })}
          </nav>

          <div className="hidden items-center gap-2 text-[10px] font-black uppercase tracking-[0.18em] text-cv-subtext xl:flex">
            <Cpu size={13} className="text-cyan-200" />
            HUD Link
            <ShieldCheck size={13} className="text-emerald-300" />
            Secure
          </div>
        </div>
      </div>

      <AnimatePresence>
        {showNotifications && (
          <motion.div
            initial={{ opacity: 0, y: -8, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -8, scale: 0.98 }}
            transition={{ duration: 0.18 }}
            className="cyber-terminal-panel absolute right-5 top-[calc(100%+10px)] z-50 w-96 max-w-[calc(100vw-2rem)] bg-[#05050a]/95 p-0"
          >
            <div className="flex items-center justify-between border-b border-cyan-300/15 px-4 py-3">
              <span className="flex items-center gap-2 text-xs font-black uppercase tracking-[0.16em] text-cyan-200">
                <RadioTower size={14} /> Command Feed
              </span>
              <span className="text-[10px] uppercase tracking-[0.18em] text-cv-subtext">{statusMessages.length} messages</span>
            </div>
            <div className="max-h-80 overflow-y-auto py-1">
              {statusMessages.length === 0 ? (
                <div className="px-4 py-5 text-xs text-cv-subtext">No notifications yet</div>
              ) : (
                statusMessages.slice(-12).reverse().map((message, index) => (
                  <div key={`${message}-${index}`} className="border-b border-cyan-300/[0.07] px-4 py-3 last:border-b-0">
                    <div className="text-xs leading-relaxed text-cv-text">{message}</div>
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
