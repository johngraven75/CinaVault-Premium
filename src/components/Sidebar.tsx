// CinaVault Premium — Build 132 Futuristic Sidebar Navigation
import React from "react";
import { motion, AnimatePresence } from "framer-motion";
import { useAppStore, TabId } from "../store/appStore";
import {
  Home,
  FolderOpen,
  Download,
  Tv,
  Server,
  Shield,
  Sliders,
  Cloud,
  Puzzle,
  Brain,
  Settings,
  ChevronLeft,
  ChevronRight,
  Router,
} from "lucide-react";

interface NavItem {
  id: TabId;
  label: string;
  icon: React.FC<{ size?: number; className?: string }>;
  signal: string;
}

const NAV_ITEMS: NavItem[] = [
  { id: "home", label: "Library", icon: Home, signal: "Archive" },
  { id: "sources", label: "Media Sources", icon: FolderOpen, signal: "Ingest" },
  { id: "downloads", label: "Downloads", icon: Download, signal: "Queue" },
  { id: "livetv", label: "Live TV", icon: Tv, signal: "Stream" },
  { id: "server", label: "Server", icon: Server, signal: "Core" },
  { id: "security", label: "Security", icon: Shield, signal: "Guard" },
  { id: "remote", label: "Remote Access", icon: Router, signal: "Relay" },
  { id: "advanced", label: "Advanced", icon: Sliders, signal: "Tune" },
  { id: "cloud", label: "Cloud & NAS", icon: Cloud, signal: "Mesh" },
  { id: "plugins", label: "Plugins", icon: Puzzle, signal: "Mods" },
  { id: "ai", label: "AI Diagnostics", icon: Brain, signal: "Neural" },
  { id: "settings", label: "Settings", icon: Settings, signal: "Config" },
];

export default function Sidebar(): JSX.Element {
  const { activeTab, setActiveTab, sidebarCollapsed, toggleSidebar } = useAppStore();

  return (
    <motion.aside
      className="cv-sidebar relative z-20 h-full shrink-0 overflow-hidden rounded-2xl border border-white/10 bg-black/20 shadow-[0_24px_70px_rgba(0,0,0,0.46)] backdrop-blur-2xl"
      animate={{ width: sidebarCollapsed ? 76 : 264 }}
      transition={{ duration: 0.32, ease: [0.16, 1, 0.3, 1] }}
    >
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_20%_0%,rgba(255,255,255,0.16),transparent_28%),radial-gradient(circle_at_70%_18%,rgba(0,234,255,0.14),transparent_32%),linear-gradient(180deg,rgba(255,255,255,0.06),rgba(255,255,255,0.015))]" />
      <div className="pointer-events-none absolute inset-y-8 right-0 w-px bg-gradient-to-b from-transparent via-[var(--cv-accent)]/45 to-transparent" />

      <div className="relative z-10 flex h-full flex-col">
        <div className="flex h-24 shrink-0 items-center gap-3 border-b border-white/10 px-3 overflow-hidden">
          <motion.div
            className="relative grid h-14 w-14 shrink-0 place-items-center rounded-2xl border border-white/15 bg-white/[0.06] shadow-[0_0_28px_rgba(0,234,255,0.18)]"
            whileHover={{ scale: 1.04, rotate: -1 }}
            transition={{ type: "spring", stiffness: 360, damping: 22 }}
          >
            <div className="absolute inset-1 rounded-xl bg-[radial-gradient(circle_at_35%_20%,rgba(255,255,255,0.32),transparent_40%)]" />
            <img
              src="/branding/cinavault-premium-mark.png"
              alt="CinaVault Premium"
              className="relative h-11 w-11 rounded-xl object-cover"
            />
          </motion.div>

          <AnimatePresence initial={false}>
            {!sidebarCollapsed && (
              <motion.div
                key="brand-copy"
                initial={{ opacity: 0, x: -12, filter: "blur(6px)" }}
                animate={{ opacity: 1, x: 0, filter: "blur(0px)" }}
                exit={{ opacity: 0, x: -12, filter: "blur(6px)" }}
                transition={{ duration: 0.22 }}
                className="min-w-0"
              >
                <div className="text-[11px] font-semibold uppercase tracking-[0.36em]" style={{ color: "var(--cv-accent)" }}>
                  Premium
                </div>
                <div className="truncate text-lg font-black tracking-tight" style={{ color: "var(--cv-text)" }}>
                  CinaVault
                </div>
                <div className="mt-1 flex items-center gap-2 text-[10px] uppercase tracking-[0.18em]" style={{ color: "var(--cv-subtext)" }}>
                  <span className="h-1.5 w-1.5 rounded-full bg-emerald-300 shadow-[0_0_12px_rgba(110,231,183,0.8)]" />
                  Media Core Online
                </div>
              </motion.div>
            )}
          </AnimatePresence>
        </div>

        <nav className="relative flex-1 overflow-y-auto px-2 py-3">
          <div className="space-y-1">
            {NAV_ITEMS.map((item) => {
              const isActive = activeTab === item.id;
              const Icon = item.icon;

              return (
                <motion.button
                  key={item.id}
                  type="button"
                  onClick={() => setActiveTab(item.id)}
                  className={`group relative flex w-full items-center gap-3 overflow-hidden rounded-xl px-3 py-2.5 text-left ${
                    isActive ? "text-white" : "text-cv-subtext hover:text-cv-text"
                  }`}
                  title={sidebarCollapsed ? item.label : undefined}
                  whileHover={{ x: sidebarCollapsed ? 0 : 3 }}
                  whileTap={{ scale: 0.985 }}
                >
                  {isActive && (
                    <motion.div
                      layoutId="sidebar-active-panel"
                      className="absolute inset-0 rounded-xl border border-white/15 bg-[linear-gradient(135deg,rgba(0,234,255,0.24),rgba(255,77,184,0.10),rgba(255,255,255,0.05))] shadow-[0_0_28px_rgba(0,234,255,0.14)]"
                      transition={{ type: "spring", stiffness: 420, damping: 34 }}
                    />
                  )}

                  <span className="absolute inset-y-2 left-0 w-px rounded-full bg-gradient-to-b from-transparent via-white/0 to-transparent group-hover:via-white/35" />
                  {isActive && (
                    <motion.span
                      layoutId="sidebar-active-rail"
                      className="absolute left-0 top-1/2 h-8 w-1 -translate-y-1/2 rounded-full bg-[var(--cv-accent)] shadow-[0_0_16px_var(--cv-accent)]"
                    />
                  )}

                  <span className="relative z-10 grid h-9 w-9 shrink-0 place-items-center rounded-xl border border-white/10 bg-black/20 shadow-[inset_0_1px_0_rgba(255,255,255,0.12)]">
                    <Icon size={18} className="shrink-0" />
                  </span>

                  <AnimatePresence initial={false}>
                    {!sidebarCollapsed && (
                      <motion.span
                        key={`${item.id}-label`}
                        initial={{ opacity: 0, x: -8 }}
                        animate={{ opacity: 1, x: 0 }}
                        exit={{ opacity: 0, x: -8 }}
                        className="relative z-10 min-w-0 flex-1"
                      >
                        <span className="block truncate text-[13px] font-semibold">{item.label}</span>
                        <span className="block text-[9px] uppercase tracking-[0.22em] opacity-60">{item.signal}</span>
                      </motion.span>
                    )}
                  </AnimatePresence>
                </motion.button>
              );
            })}
          </div>
        </nav>

        <button
          type="button"
          onClick={toggleSidebar}
          className="relative z-10 m-2 flex h-11 items-center justify-center rounded-xl border border-white/10 bg-white/[0.04] text-cv-subtext shadow-[inset_0_1px_0_rgba(255,255,255,0.08)] hover:bg-white/[0.08] hover:text-cv-text"
          title={sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
        >
          {sidebarCollapsed ? <ChevronRight size={16} /> : <ChevronLeft size={16} />}
        </button>
      </div>
    </motion.aside>
  );
}
