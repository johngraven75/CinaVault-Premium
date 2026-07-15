// Build 140 Futuristic Sidebar Navigation
// Build 157 Futuristic Sidebar Navigation
// CinaVault Premium — Media Center Sidebar Navigation
import type { JSX } from "react";
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
  Film,
  type LucideIcon,
} from "lucide-react";

interface NavItem {
  id: TabId;
  label: string;
  icon: LucideIcon;
  group: "Media" | "System" | "Tools";
  signal: string;
}

const NAV_ITEMS: NavItem[] = [
  { id: "home", label: "Home", icon: Home, group: "Media", signal: "Featured" },
  { id: "sources", label: "Sources", icon: FolderOpen, group: "Media", signal: "Library" },
  { id: "downloads", label: "Downloads", icon: Download, group: "Media", signal: "Queue" },
  { id: "livetv", label: "Live TV", icon: Tv, group: "Media", signal: "Channels" },
  { id: "server", label: "Server", icon: Server, group: "System", signal: "Core" },
  { id: "security", label: "Security", icon: Shield, group: "System", signal: "Guard" },
  { id: "remote", label: "Remote", icon: Router, group: "System", signal: "Access" },
  { id: "advanced", label: "Advanced", icon: Sliders, group: "Tools", signal: "Tune" },
  { id: "cloud", label: "Cloud NAS", icon: Cloud, group: "Tools", signal: "Mesh" },
  { id: "plugins", label: "Plugins", icon: Puzzle, group: "Tools", signal: "Add-ons" },
  { id: "ai", label: "AI", icon: Brain, group: "Tools", signal: "Assist" },
  { id: "settings", label: "Settings", icon: Settings, group: "System", signal: "Config" },
];

function groupItems(group: NavItem["group"]): NavItem[] {
  return NAV_ITEMS.filter(item => item.group === group);
}

function NavButton({ item, collapsed }: { item: NavItem; collapsed: boolean }): JSX.Element {
  const { activeTab, setActiveTab } = useAppStore();
  const isActive = activeTab === item.id;
  const Icon = item.icon;

  return (
    <motion.button
      key={item.id}
      type="button"
      onClick={() => setActiveTab(item.id)}
      className={`group relative flex w-full items-center gap-3 overflow-hidden rounded-2xl px-3 py-3 text-left outline-none transition-colors focus-visible:ring-2 focus-visible:ring-cv-accent/70 ${
        isActive ? "text-white" : "text-cv-subtext hover:text-cv-text"
      }`}
      title={collapsed ? item.label : undefined}
      whileHover={{ x: collapsed ? 0 : 4, scale: collapsed ? 1.04 : 1 }}
      whileTap={{ scale: 0.98 }}
    >
      {isActive && (
        <motion.div
          layoutId="media-center-active-nav"
          className="sidebar-active-panel absolute inset-0 rounded-2xl border border-white/20 bg-[linear-gradient(90deg,rgba(0,234,255,0.36),rgba(255,255,255,0.08)_48%,rgba(0,0,0,0.08))] shadow-[0_0_35px_rgba(0,234,255,0.22)]"
          transition={{ type: "spring", stiffness: 460, damping: 36 }}
        />
      )}

      {isActive && (
        <motion.span
          layoutId="media-center-active-rail"
          className="sidebar-active-rail absolute left-0 top-1/2 h-10 w-1 -translate-y-1/2 rounded-r-full bg-cyan-200 shadow-[0_0_18px_rgba(125,249,255,0.95)]"
        />
      )}

      <span className="relative z-10 grid h-11 w-11 shrink-0 place-items-center rounded-2xl border border-white/10 bg-black/30 shadow-[inset_0_1px_0_rgba(255,255,255,0.14)] group-hover:border-white/25">
        <Icon size={20} className="shrink-0" />
      </span>

      <AnimatePresence initial={false}>
        {!collapsed && (
          <motion.span
            key={`${item.id}-label`}
            initial={{ opacity: 0, x: -8, filter: "blur(6px)" }}
            animate={{ opacity: 1, x: 0, filter: "blur(0px)" }}
            exit={{ opacity: 0, x: -8, filter: "blur(6px)" }}
            transition={{ duration: 0.2 }}
            className="relative z-10 min-w-0 flex-1"
          >
            <span className="block truncate text-[14px] font-bold tracking-tight">{item.label}</span>
            <span className="block text-[9px] uppercase tracking-[0.24em] opacity-65">{item.signal}</span>
          </motion.span>
        )}
      </AnimatePresence>
    </motion.button>
  );
}

export default function Sidebar(): JSX.Element {
  const { sidebarCollapsed, toggleSidebar } = useAppStore();
  const groups: NavItem["group"][] = ["Media", "System", "Tools"];

  return (
    <motion.aside
      className="cv-sidebar relative z-20 h-full shrink-0 overflow-hidden rounded-[28px] border border-white/10 bg-[#08111f]/88 shadow-[0_24px_85px_rgba(0,0,0,0.58)] backdrop-blur-2xl"
      animate={{ width: sidebarCollapsed ? 82 : 286 }}
      transition={{ duration: 0.32, ease: [0.16, 1, 0.3, 1] }}
    >
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_30%_0%,rgba(59,196,255,0.28),transparent_30%),linear-gradient(180deg,rgba(255,255,255,0.07),rgba(255,255,255,0.018))]" />
      <div className="pointer-events-none absolute inset-y-8 right-0 w-px bg-gradient-to-b from-transparent via-cyan-200/45 to-transparent" />

      <div className="relative z-10 flex h-full flex-col">
        <div className="flex h-28 shrink-0 items-center gap-3 overflow-hidden border-b border-white/10 px-3">
          <motion.div
            className="relative grid h-16 w-16 shrink-0 place-items-center rounded-[24px] border border-white/15 bg-white/[0.06] shadow-[0_0_34px_rgba(0,234,255,0.20)]"
            whileHover={{ scale: 1.04, rotate: -1 }}
            transition={{ type: "spring", stiffness: 360, damping: 22 }}
          >
            <div className="absolute inset-1 rounded-[20px] bg-[radial-gradient(circle_at_35%_20%,rgba(255,255,255,0.34),transparent_42%)]" />
            <img
              src="/branding/cinavault-logo.png"
              alt="CinaVault Premium"
              className="relative h-12 w-12 rounded-2xl object-cover"
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
                <div className="flex items-center gap-2 text-[10px] font-bold uppercase tracking-[0.34em] text-cyan-200">
                  <Film size={12} /> Media Center
                </div>
                <div className="truncate text-2xl font-black tracking-tight" style={{ color: "var(--cv-text)" }}>
                  CinaVault
                </div>
                <div className="mt-1 flex items-center gap-2 text-[10px] uppercase tracking-[0.18em]" style={{ color: "var(--cv-subtext)" }}>
                  <span className="h-1.5 w-1.5 rounded-full bg-emerald-300 shadow-[0_0_12px_rgba(110,231,183,0.8)]" />
                  Premium Online
                </div>
              </motion.div>
            )}
          </AnimatePresence>
        </div>

        <nav className="relative flex-1 overflow-y-auto px-2 py-3">
          {groups.map(group => (
            <div key={group} className="mb-4 last:mb-0">
              {!sidebarCollapsed && (
                <div className="px-3 pb-1 text-[10px] font-bold uppercase tracking-[0.24em] text-cv-subtext/70">
                  {group}
                </div>
              )}
              <div className="space-y-1">
                {groupItems(group).map(item => (
                  <NavButton key={item.id} item={item} collapsed={sidebarCollapsed} />
                ))}
              </div>
            </div>
          ))}
        </nav>

        <button
          type="button"
          onClick={toggleSidebar}
          className="relative z-10 m-2 flex h-12 items-center justify-center rounded-2xl border border-white/10 bg-white/[0.04] text-cv-subtext shadow-[inset_0_1px_0_rgba(255,255,255,0.08)] hover:bg-white/[0.08] hover:text-cv-text focus-visible:ring-2 focus-visible:ring-cv-accent/70"
          title={sidebarCollapsed ? "Expand navigation" : "Collapse navigation"}
        >
          {sidebarCollapsed ? <ChevronRight size={17} /> : <ChevronLeft size={17} />}
        </button>
      </div>
    </motion.aside>
  );
}
