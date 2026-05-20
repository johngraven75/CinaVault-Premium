// CinaVault Premium — Animated Header with Starfield & Nebula
import React, { useRef, useEffect, useState } from "react";
import { motion } from "framer-motion";
import { useAppStore, TabId } from "../store/appStore";
import { Search, Bell, Maximize2 } from "lucide-react";
import { getUnreadStatusMessages } from "../utils/pluginUiSafety";

const TAB_LABELS: Record<TabId, string> = {
  home: "Library", sources: "Media Sources", downloads: "Downloads",
  livetv: "Live TV", server: "Server", security: "Security",
  remote: "Remote Access",
  advanced: "Advanced", cloud: "Cloud & NAS", duplicates: "Duplicate Finder", plugins: "Plugins & Metadata",
  ai: "AI Diagnostics", settings: "Settings",
};

export default function Header() {
  const { activeTab, searchQuery, setSearchQuery, statusMessages } = useAppStore();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const mouseRef = useRef({ x: 0, y: 0 });
  const [clock, setClock] = useState(new Date());
  const [showNotifications, setShowNotifications] = useState(false);
  const [lastReadMessageIndex, setLastReadMessageIndex] = useState(0);
  const unreadMessages = getUnreadStatusMessages(statusMessages, lastReadMessageIndex);

  // Starfield + Nebula + Comets animation
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d")!;
    let animId: number;
    let stars: { x: number; y: number; z: number; size: number }[] = [];
    let comets: { x: number; y: number; speed: number; length: number; angle: number; life: number; maxLife: number }[] = [];

    const resize = () => {
      canvas.width = canvas.offsetWidth * 2;
      canvas.height = canvas.offsetHeight * 2;
      ctx.scale(2, 2);
    };
    resize();

    // Init stars
    for (let i = 0; i < 120; i++) {
      stars.push({
        x: Math.random() * canvas.offsetWidth,
        y: Math.random() * canvas.offsetHeight,
        z: Math.random(),
        size: Math.random() * 1.5 + 0.3,
      });
    }

    const spawnComet = () => {
      if (comets.length < 2 && Math.random() < 0.008) {
        comets.push({
          x: -20, y: Math.random() * canvas.offsetHeight * 0.6,
          speed: 2 + Math.random() * 3,
          length: 40 + Math.random() * 60,
          angle: -0.15 + Math.random() * 0.3,
          life: 0, maxLife: 120 + Math.random() * 80,
        });
      }
    };

    const draw = () => {
      const w = canvas.offsetWidth;
      const h = canvas.offsetHeight;
      ctx.clearRect(0, 0, w, h);

      // Nebula haze
      const grd = ctx.createRadialGradient(w * 0.7, h * 0.3, 0, w * 0.7, h * 0.3, w * 0.6);
      grd.addColorStop(0, "rgba(167,139,250,0.06)");
      grd.addColorStop(0.5, "rgba(139,92,246,0.03)");
      grd.addColorStop(1, "transparent");
      ctx.fillStyle = grd;
      ctx.fillRect(0, 0, w, h);

      // Parallax offset from mouse
      const mx = (mouseRef.current.x - w / 2) * 0.02;
      const my = (mouseRef.current.y - h / 2) * 0.02;

      // Stars
      stars.forEach((s) => {
        const px = s.x + mx * s.z;
        const py = s.y + my * s.z;
        const alpha = 0.3 + s.z * 0.5 + Math.sin(Date.now() * 0.002 + s.x) * 0.15;
        ctx.beginPath();
        ctx.arc(px, py, s.size, 0, Math.PI * 2);
        ctx.fillStyle = `rgba(200,200,255,${alpha})`;
        ctx.fill();
      });

      // Comets
      spawnComet();
      comets = comets.filter((c) => c.life < c.maxLife);
      comets.forEach((c) => {
        c.x += c.speed * Math.cos(c.angle);
        c.y += c.speed * Math.sin(c.angle);
        c.life++;

        const fadeIn = Math.min(c.life / 15, 1);
        const fadeOut = Math.max(1 - (c.life - c.maxLife + 20) / 20, 0);
        const alpha = fadeIn * fadeOut * 0.7;

        const grad = ctx.createLinearGradient(
          c.x, c.y,
          c.x - c.length * Math.cos(c.angle),
          c.y - c.length * Math.sin(c.angle)
        );
        grad.addColorStop(0, `rgba(167,139,250,${alpha})`);
        grad.addColorStop(0.5, `rgba(192,132,252,${alpha * 0.4})`);
        grad.addColorStop(1, "transparent");

        ctx.beginPath();
        ctx.moveTo(c.x, c.y);
        ctx.lineTo(c.x - c.length * Math.cos(c.angle), c.y - c.length * Math.sin(c.angle));
        ctx.strokeStyle = grad;
        ctx.lineWidth = 1.5;
        ctx.stroke();

        // Head glow
        ctx.beginPath();
        ctx.arc(c.x, c.y, 2, 0, Math.PI * 2);
        ctx.fillStyle = `rgba(255,255,255,${alpha})`;
        ctx.fill();
      });

      animId = requestAnimationFrame(draw);
    };

    draw();
    const handleMouse = (e: MouseEvent) => {
      const rect = canvas.getBoundingClientRect();
      mouseRef.current = { x: e.clientX - rect.left, y: e.clientY - rect.top };
    };
    canvas.addEventListener("mousemove", handleMouse);

    return () => {
      cancelAnimationFrame(animId);
      canvas.removeEventListener("mousemove", handleMouse);
    };
  }, []);

  useEffect(() => {
    const timer = setInterval(() => setClock(new Date()), 1000);
    return () => clearInterval(timer);
  }, []);

  const toggleFullscreen = async () => {
    try {
      if (document.fullscreenElement) {
        await document.exitFullscreen();
      } else {
        await document.documentElement.requestFullscreen();
      }
    } catch {}
  };

  return (
    <header className="cv-header relative h-16 shrink-0 flex items-center border-b border-white/5 overflow-visible">
      {/* Animated background */}
      <canvas
        ref={canvasRef}
        className="absolute inset-0 w-full h-full"
        style={{ pointerEvents: "auto" }}
      />

      {/* Content overlay */}
      <div className="relative z-10 flex items-center justify-between w-full px-5">
        {/* Tab title */}
        <div className="flex items-center gap-3">
          <img
            src="/branding/cinavault-premium-mark.png"
            alt="CinaVault brand"
            className="w-9 h-9 rounded-md object-cover border border-white/10 shadow-[0_8px_18px_rgba(0,0,0,0.35)]"
          />
          <motion.h1
            key={activeTab}
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            className="text-lg font-bold tracking-tight"
            style={{ color: "var(--cv-text)" }}
          >
            {TAB_LABELS[activeTab]}
          </motion.h1>
          <div className="h-4 w-px bg-white/10" />
          <span className="cv-header-chip text-xs font-medium">
            Cinavault Premium Media Server
          </span>
        </div>

        {/* Search & Actions */}
        <div className="flex items-center gap-3">
          <div className="relative cv-command-bar">
            <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-cv-subtext" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search library..."
              className="cv-input pl-9 w-52 text-xs"
            />
          </div>

          <div className="text-[11px] text-cv-subtext/80 font-mono tracking-wide min-w-[74px] text-right">
            {clock.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" })}
          </div>

          <button
            onClick={toggleFullscreen}
            className="w-8 h-8 rounded-lg flex items-center justify-center bg-white/5 hover:bg-white/10 transition-colors"
            title="Toggle fullscreen"
          >
            <Maximize2 size={14} className="text-cv-subtext" />
          </button>

          <button
            onClick={() => {
              setShowNotifications((open) => !open);
              setLastReadMessageIndex(Math.max(0, statusMessages.length - 1));
            }}
            className="relative w-8 h-8 rounded-lg flex items-center justify-center bg-white/5 hover:bg-white/10 transition-colors"
            title="Show notifications"
          >
            <Bell size={14} className="text-cv-subtext" />
            {unreadMessages.length > 0 && (
              <span className="absolute right-1.5 top-1.5 w-1.5 h-1.5 rounded-full bg-cyan-300 shadow-[0_0_10px_rgba(34,211,238,0.8)]" />
            )}
          </button>
        </div>
      </div>

      {showNotifications && (
        <div className="absolute right-5 top-[calc(100%+8px)] z-50 w-80 max-w-[calc(100vw-2rem)] rounded-lg border border-white/10 bg-[#10131d]/95 shadow-2xl backdrop-blur-xl overflow-hidden">
          <div className="flex items-center justify-between px-3 py-2 border-b border-white/10">
            <span className="text-xs font-semibold" style={{ color: "var(--cv-text)" }}>Notifications</span>
            <span className="text-[10px]" style={{ color: "var(--cv-subtext)" }}>{statusMessages.length} messages</span>
          </div>
          <div className="max-h-72 overflow-y-auto py-1">
            {statusMessages.length === 0 ? (
              <div className="px-3 py-4 text-xs" style={{ color: "var(--cv-subtext)" }}>No notifications yet</div>
            ) : (
              statusMessages.slice(-12).reverse().map((message, index) => (
                <div key={`${message}-${index}`} className="px-3 py-2 border-b border-white/[0.04] last:border-b-0">
                  <div className="text-xs leading-relaxed" style={{ color: "var(--cv-text)" }}>{message}</div>
                </div>
              ))
            )}
          </div>
        </div>
      )}
    </header>
  );
}
