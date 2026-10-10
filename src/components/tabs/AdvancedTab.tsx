// CinaVault Premium — Advanced Tab (Feature Matrix + Media Requests)
import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion } from "framer-motion";
import { useAppStore } from "../../store/appStore";
import {
  Sliders,
  Zap,
  MessageSquare,
  ExternalLink,
  ChevronDown,
  ChevronRight,
  Settings2,
} from "lucide-react";
import TabBanner from "../experience/TabBanner";
import DuplicateFinder from "../library/DuplicateFinder";
import { FEATURE_MATRIX } from "../../features/featureCatalog";
import {
  featureConfig,
  isFeatureOn,
  saveFeature,
  type FeatureKey,
} from "../../features/featureFlags";
import type { FeaturePanel } from "../../features/panels/types";

const REQUESTS_SETTING_KEY = "media_request_queue";

// Settings panels live in src/features/panels/<switch key>.tsx.
const PANEL_MODULES = import.meta.glob<{ default: FeaturePanel }>("../../features/panels/*.tsx", { eager: true });
const FEATURE_PANELS: Partial<Record<string, FeaturePanel>> = Object.fromEntries(
  Object.entries(PANEL_MODULES).map(([path, module]) => [path.replace(/^.*\/|\.tsx$/g, ""), module.default]),
);

export default function AdvancedTab() {
  const { featureSettings, addStatusMessage } = useAppStore();
  const [openPanels, setOpenPanels] = useState<Set<string>>(new Set());
  const [expandedCats, setExpandedCats] = useState<Set<string>>(
    new Set(FEATURE_MATRIX.map((c) => c.name)),
  );
  const [requestQueue, setRequestQueue] = useState<any[]>([]);
  const [newRequest, setNewRequest] = useState({
    title: "",
    type: "movie",
    requester: "",
  });

  const toggleCat = (name: string) => {
    setExpandedCats((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  };

  const isEnabled = (key: FeatureKey) => isFeatureOn(featureSettings, key);
  const togglePanel = (key: string) =>
    setOpenPanels((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });

  // Requests used to live only in memory; keep them in the settings table so
  // they survive a restart.
  useEffect(() => {
    invoke<string | null>("get_setting", { key: REQUESTS_SETTING_KEY })
      .then((raw) => {
        if (!raw) return;
        const parsed = JSON.parse(raw);
        if (Array.isArray(parsed)) setRequestQueue(parsed);
      })
      .catch(() => {});
  }, []);
  const saveRequests = (next: any[]) => {
    setRequestQueue(next);
    invoke("set_setting", { key: REQUESTS_SETTING_KEY, value: JSON.stringify(next) }).catch((error) =>
      addStatusMessage(`Could not save media requests: ${error}`),
    );
  };

  const handleToggle = async (key: FeatureKey, label: string) => {
    const enabled = !isEnabled(key);
    try {
      // Keep the switch's saved settings when turning it on or off.
      await saveFeature(key, enabled, featureSettings[key]?.config ?? {});
      addStatusMessage(`${label} ${enabled ? "on" : "off"}`);
    } catch (error) {
      addStatusMessage(`${label}: update failed (${String(error)})`);
    }
  };

  const saveConfig = async (key: FeatureKey, label: string, next: object) => {
    try {
      await saveFeature(key, isEnabled(key), { ...featureConfig(featureSettings, key, {}), ...next });
    } catch (error) {
      addStatusMessage(`${label}: settings not saved (${String(error)})`);
      throw error;
    }
  };

  const addRequest = () => {
    if (!newRequest.title) return;
    saveRequests([
      {
        ...newRequest,
        id: Date.now(),
        status: "pending",
        created: new Date().toLocaleString(),
      },
      ...requestQueue,
    ]);
    setNewRequest({ title: "", type: "movie", requester: "" });
    addStatusMessage(`Request added: ${newRequest.title}`);
  };

  return (
    <div className="space-y-5">
      <TabBanner icon={Sliders} eyebrow="Expert Systems" title="Control Lab" subtitle="Deep diagnostics, repair controls, platform tuning, and advanced operational tooling." accent="from-orange-300/28 to-fuchsia-500/10" accentText="text-orange-100" />
      <DuplicateFinder />

      {/* Feature Matrix */}
      <div className="glass-panel p-5">
        <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
          <Zap size={16} className="text-cv-accent" /> Feature Matrix
        </h3>
        <div className="space-y-2">
          {FEATURE_MATRIX.map((cat) => (
            <div
              key={cat.name}
              className="glass-panel-2 rounded-lg overflow-hidden"
            >
              <button
                onClick={() => toggleCat(cat.name)}
                className="w-full px-4 py-3 flex items-center justify-between hover:bg-white/[0.03] transition-colors"
              >
                <span className="text-xs font-bold uppercase tracking-wider text-cv-accent">
                  {cat.name}
                </span>
                <div className="flex items-center gap-2">
                  <span className="text-[10px] text-cv-subtext">
                    {
                      cat.features.filter((f) => isEnabled(f.key)).length
                    }
                    /{cat.features.length}
                  </span>
                  {expandedCats.has(cat.name) ? (
                    <ChevronDown size={14} />
                  ) : (
                    <ChevronRight size={14} />
                  )}
                </div>
              </button>
              {expandedCats.has(cat.name) && (
                <motion.div
                  initial={{ opacity: 0, height: 0 }}
                  animate={{ opacity: 1, height: "auto" }}
                  className="px-4 pb-3 space-y-1"
                >
                  {cat.features.map((feature) => {
                    const Panel = FEATURE_PANELS[feature.key];
                    const panelOpen = openPanels.has(feature.key);
                    return (
                      <div key={feature.key} className="rounded hover:bg-white/[0.02]">
                        <div className="flex items-center justify-between gap-2 py-1.5 px-2">
                          <div className="flex-1 min-w-0">
                            <div className="text-sm">{feature.label}</div>
                            <div className="text-[10px] text-cv-subtext">{feature.description}</div>
                          </div>
                          {Panel && (
                            <button
                              type="button"
                              onClick={() => togglePanel(feature.key)}
                              aria-expanded={panelOpen}
                              aria-label={`${feature.label} settings`}
                              title="Settings"
                              className={`cv-btn cv-btn-secondary px-1.5 py-1 ${panelOpen ? "text-cv-accent" : ""}`}
                            >
                              <Settings2 size={12} />
                            </button>
                          )}
                          <button
                            type="button"
                            role="switch"
                            aria-checked={isEnabled(feature.key)}
                            aria-label={feature.label}
                            data-feature={feature.key}
                            className={`cv-toggle ${isEnabled(feature.key) ? "active" : ""}`}
                            onClick={() => void handleToggle(feature.key, feature.label)}
                          />
                        </div>
                        {Panel && panelOpen && (
                          <div className="mx-2 mb-2 rounded-lg border border-white/10 bg-black/20 p-3 text-xs" data-feature-panel={feature.key}>
                            <Panel
                              enabled={isEnabled(feature.key)}
                              config={featureConfig(featureSettings, feature.key, {})}
                              setConfig={(next) => saveConfig(feature.key, feature.label, next)}
                            />
                          </div>
                        )}
                      </div>
                    );
                  })}
                </motion.div>
              )}
            </div>
          ))}
        </div>
      </div>

      {/* Media Requests & Automation */}
      <div className="glass-panel p-5">
        <h3 className="text-sm font-bold mb-4 flex items-center gap-2">
          <MessageSquare size={16} className="text-cv-accent" /> Media Requests
          & Automation
        </h3>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-5">
          {/* Request Queue */}
          <div>
            <label className="section-label">New Request</label>
            <div className="space-y-2 mb-3">
              <input
                value={newRequest.title}
                onChange={(e) =>
                  setNewRequest({ ...newRequest, title: e.target.value })
                }
                className="cv-input"
                placeholder="Title to request..."
              />
              <div className="flex gap-2">
                <select
                  value={newRequest.type}
                  onChange={(e) =>
                    setNewRequest({ ...newRequest, type: e.target.value })
                  }
                  className="cv-select flex-1"
                >
                  <option value="movie">Movie</option>
                  <option value="tvshow">TV Show</option>
                  <option value="music">Music</option>
                </select>
                <input
                  value={newRequest.requester}
                  onChange={(e) =>
                    setNewRequest({ ...newRequest, requester: e.target.value })
                  }
                  className="cv-input flex-1"
                  placeholder="Requester"
                />
              </div>
              <button
                onClick={addRequest}
                className="cv-btn cv-btn-primary text-xs w-full"
              >
                Add Request
              </button>
            </div>

            {requestQueue.length > 0 && (
              <div className="glass-panel-2 rounded-lg max-h-48 overflow-y-auto divide-y divide-white/5">
                {requestQueue.map((req) => (
                  <div key={req.id} className="px-3 py-2 text-xs flex items-start justify-between gap-2">
                    <div className="min-w-0">
                      <div className="font-semibold truncate">{req.title}</div>
                      <div className="text-cv-subtext">
                        {req.type} - {req.status} - {req.requester || "Anonymous"}
                      </div>
                    </div>
                    <button
                      type="button"
                      onClick={() => saveRequests(requestQueue.filter((r) => r.id !== req.id))}
                      className="cv-btn cv-btn-secondary shrink-0 px-2 py-0.5 text-[10px]"
                    >
                      Remove
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Integrations */}
          <div>
            <label className="section-label">Integrations</label>
            <div className="space-y-2">
              {[
                {
                  name: "Overseerr",
                  desc: "Media request management",
                  url: "https://overseerr.dev",
                },
                {
                  name: "MS-C Requests",
                  desc: "MS-C request management",
                  url: "https://github.com/Fallenbagel/jellyseerr",
                },
              ].map((int) => (
                <div
                  key={int.name}
                  className="glass-panel-2 p-3 rounded-lg flex items-center justify-between"
                >
                  <div>
                    <div className="text-sm font-semibold">{int.name}</div>
                    <div className="text-[10px] text-cv-subtext">
                      {int.desc}
                    </div>
                  </div>
                  <button
                    onClick={() => window.open?.(int.url)}
                    className="cv-btn cv-btn-secondary text-[10px] py-1 px-2"
                  >
                    <ExternalLink size={10} /> Open
                  </button>
                </div>
              ))}
            </div>

          </div>
        </div>
      </div>
    </div>
  );
}
