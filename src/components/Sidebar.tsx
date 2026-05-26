// CinaVault Premium — Sidebar Navigation
import React from "react";
import { motion } from "framer-motion";
import { useAppStore, TabId } from "../store/appStore";
import {
  Home, FolderOpen, Download, Tv, Server, Shield, Sliders,
  Cloud, Copy, Puzzle, Brain, Settings, ChevronLeft, ChevronRight, Router
} from "lucide-react";

interface NavItem {
  id: TabId;
  label: string;
  icon: React.FC<any>;
  accent?: string;
}

const NAV_ITEMS: NavItem[] = [
  { id: "home", label: "Library", icon: Home },
  { id: "sources", label: "Media Sources", icon: FolderOpen },
  { id: "downloads", label: "Downloads", icon: Download },
  { id: "livetv", label: "Live TV", icon: Tv },
  { id: "server", label: "Server", icon: Server },
  { id: "security", label: "Security", icon: Shield },
  { id: "remote", label: "Remote Access", icon: Router },
  { id: "advanced", label: "Advanced", icon: Sliders },
  { id: "cloud", label: "Cloud & NAS", icon: Cloud },
  { id: "duplicates", label: "Duplicate Finder", icon: Copy },
  { id: "plugins", label: "Plugins", icon: Puzzle },
  { id: "ai", label: "AI Diagnostics", icon: Brain },
  { id: "settings", label: "Settings", icon: Settings },
];

export default function Sidebar() {
  const { activeTab, setActiveTab, sidebarCollapsed, toggleSidebar } = useAppStore();

  return (
    <motion.aside
      className="cv-sidebar h-full flex flex-col border-r border-white/5 relative z-10"
      style={{ background: "var(--cv-panel)" }}
      animate={{ width: sidebarCollapsed ? 64 : 220 }}
      transition={{ duration: 0.3, ease: [0.4, 0, 0.2, 1] }}
    >
      {/* Logo / Brand */}
      <div className="h-20 flex items-center px-3 gap-3 border-b border-white/5 shrink-0 overflow-hidden">
        <img
          src="/branding/cinavault-premium-brand-full.png"
          alt="CinaVault Premium"
          className="w-12 h-12 rounded-xl object-contain border border-white/10 bg-black/20 shadow-[0_10px_22px_rgba(0,0,0,0.45)] shrink-0"
        />
        {!sidebarCollapsed && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="flex flex-col min-w-0"
          >
            <span className="text-sm font-bold tracking-tight truncate" style={{ color: "var(--cv-text)" }}>
              CinaVault
            </span>
            <span className="text-[10px] font-medium tracking-wider uppercase" style={{ color: "var(--cv-accent)" }}>
              Premium Media Server
            </span>
          </motion.div>
        )}
      </div>

      {/* Navigation Items */}
      <nav className="flex-1 overflow-y-auto py-2 px-2 space-y-0.5">
        {NAV_ITEMS.map((item) => {
          const isActive = activeTab === item.id;
          const Icon = item.icon;

          return (
            <button
              key={item.id}
              onClick={() => setActiveTab(item.id)}
              className={`
                cv-nav-item
                w-full flex items-center gap-3 px-3 py-2 rounded-lg text-left
                transition-all duration-200 relative group
                ${isActive
                  ? "text-white"
                  : "text-cv-subtext hover:text-cv-text hover:bg-white/5"
                }
              `}
              title={sidebarCollapsed ? item.label : undefined}
            >
              {/* Active indicator */}
              {isActive && (
                <motion.div
                  layoutId="sidebar-active"
                  className="absolute inset-0 rounded-lg"
                  style={{
                    background: "linear-gradient(135deg, rgba(167,139,250,0.15), rgba(167,139,250,0.05))",
                    border: "1px solid rgba(167,139,250,0.2)",
                  }}
                  transition={{ type: "spring", stiffness: 400, damping: 30 }}
                />
              )}

              {/* Accent line */}
              {isActive && (
                <motion.div
                  layoutId="sidebar-accent"
                  className="absolute left-0 top-1/2 -translate-y-1/2 w-[3px] h-5 rounded-full"
                  style={{
                    background: "linear-gradient(180deg, var(--cv-neon-1), var(--cv-accent))",
                    boxShadow: "0 0 8px var(--cv-neon-1)",
                  }}
                />
              )}

              <Icon size={18} className="shrink-0 relative z-10" />

              {!sidebarCollapsed && (
                <span className="text-[13px] font-medium truncate relative z-10">
                  {item.label}
                </span>
              )}
            </button>
          );
        })}
      </nav>

      {/* Collapse Toggle */}
      <button
        onClick={toggleSidebar}
        className="h-10 flex items-center justify-center border-t border-white/5 text-cv-subtext hover:text-cv-text transition-colors cv-sidebar-toggle"
      >
        {sidebarCollapsed ? <ChevronRight size={16} /> : <ChevronLeft size={16} />}
      </button>
    </motion.aside>
  );
}
