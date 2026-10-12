// CinaVault Premium — Live TV settings (the player's gear button).
//
// "Player" holds per-device playback preferences. "Source" edits the login and
// request headers of the source the channel belongs to: the fields follow the
// provider's login type (Xtream Codes, M3U playlist, Stalker MAC portal).
import { useEffect, useState } from "react";
import { Loader2, Save, Settings2, SlidersHorizontal, Tv, X } from "lucide-react";
import {
  DEFAULT_PLAYER_SETTINGS,
  FIT_OPTIONS,
  LATENCY_OPTIONS,
  QUALITY_OPTIONS,
  type PlayerSettings,
} from "../utils/iptvPlayerSettings";
import { USER_AGENT_PRESETS, loginTypeInfo, type IptvSourceForm } from "../utils/iptvSource";

export interface SettingsSource {
  id: number;
  name: string;
  server_url: string;
  username: string;
  source_kind?: string;
  user_agent?: string | null;
  referrer?: string | null;
  has_secret?: boolean;
}

export interface ChannelHeaders {
  userAgent: string | null;
  referrer: string | null;
}

export type SettingsTab = "player" | "source";

export function sourceToForm(source: SettingsSource): IptvSourceForm {
  return {
    kind: loginTypeInfo(source.source_kind).id,
    name: source.name,
    serverUrl: source.server_url,
    username: source.username,
    secret: "",
    userAgent: source.user_agent ?? "",
    referrer: source.referrer ?? "",
  };
}

interface SourceFieldsProps {
  form: IptvSourceForm;
  onChange: (form: IptvSourceForm) => void;
  /** Editing a saved source: an empty password / MAC keeps the saved one. */
  hasSavedSecret?: boolean;
  idPrefix: string;
}

/** Login fields for one provider type plus the request header overrides. */
export function IptvSourceFields({ form, onChange, hasSavedSecret = false, idPrefix }: Readonly<SourceFieldsProps>) {
  const info = loginTypeInfo(form.kind);
  const set = (patch: Partial<IptvSourceForm>) => onChange({ ...form, ...patch });
  const id = (field: string) => `${idPrefix}-${field}`;
  const preset = USER_AGENT_PRESETS.find((option) => option.value === form.userAgent.trim());
  return (
    <div className="space-y-3">
      <p className="text-[11px] text-cv-subtext">{info.hint}</p>
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        <div>
          <label htmlFor={id("name")} className="section-label">Name</label>
          <input id={id("name")} value={form.name} onChange={(e) => set({ name: e.target.value })} className="cv-input" placeholder={info.label} />
        </div>
        <div>
          <label htmlFor={id("server")} className="section-label">{info.serverLabel}</label>
          <input id={id("server")} value={form.serverUrl} onChange={(e) => set({ serverUrl: e.target.value })} className="cv-input" placeholder={info.serverPlaceholder} />
        </div>
        {info.needsUsername && (
          <div>
            <label htmlFor={id("username")} className="section-label">Username</label>
            <input id={id("username")} value={form.username} onChange={(e) => set({ username: e.target.value })} className="cv-input" autoComplete="off" />
          </div>
        )}
        {info.secretLabel && (
          <div>
            <label htmlFor={id("secret")} className="section-label">{info.secretLabel}</label>
            <input
              id={id("secret")}
              type={form.kind === "stalker" ? "text" : "password"}
              value={form.secret}
              onChange={(e) => set({ secret: e.target.value })}
              className="cv-input"
              autoComplete="off"
              placeholder={hasSavedSecret ? "Saved. Leave blank to keep it." : info.secretPlaceholder}
            />
          </div>
        )}
      </div>
      <fieldset className="rounded-lg border border-white/5 p-3">
        <legend className="px-1 text-[10px] font-bold uppercase tracking-wider text-cv-subtext">Request headers</legend>
        <p className="mb-2 text-[11px] text-cv-subtext">
          Some providers only serve streams to a known player or from their own site. These apply to every channel in this source unless the playlist sets its own for a channel.
        </p>
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
          <div>
            <label htmlFor={id("agent-preset")} className="section-label">User-Agent</label>
            <select
              id={id("agent-preset")}
              value={preset ? preset.value : "custom"}
              onChange={(e) => e.target.value !== "custom" && set({ userAgent: e.target.value })}
              className="cv-input mb-2"
            >
              {USER_AGENT_PRESETS.map((option) => (
                <option key={option.label} value={option.value}>{option.label}</option>
              ))}
              <option value="custom">Custom…</option>
            </select>
            <input
              aria-label="Custom User-Agent"
              value={form.userAgent}
              onChange={(e) => set({ userAgent: e.target.value })}
              className="cv-input font-mono text-[11px]"
              placeholder="Leave blank for the default"
            />
          </div>
          <div>
            <label htmlFor={id("referrer")} className="section-label">Referer</label>
            <input
              id={id("referrer")}
              value={form.referrer}
              onChange={(e) => set({ referrer: e.target.value })}
              className="cv-input font-mono text-[11px]"
              placeholder="https://provider.com/"
            />
          </div>
        </div>
      </fieldset>
    </div>
  );
}

interface PanelProps {
  tab: SettingsTab;
  onTabChange: (tab: SettingsTab) => void;
  settings: PlayerSettings;
  onSettingsChange: (settings: PlayerSettings) => void;
  /** The source to edit; null hides the Source tab (direct stream URLs). */
  source: SettingsSource | null;
  /** Headers the current channel is playing with, when known. */
  channelHeaders?: ChannelHeaders | null;
  onSaveSource: (id: number, form: IptvSourceForm) => Promise<void>;
  onClose: () => void;
  /** "overlay" sits inside the player (works in full screen); "modal" covers the page. */
  variant?: "overlay" | "modal";
}

export default function IPTVSettingsPanel({
  tab,
  onTabChange,
  settings,
  onSettingsChange,
  source,
  channelHeaders,
  onSaveSource,
  onClose,
  variant = "overlay",
}: Readonly<PanelProps>) {
  const [form, setForm] = useState<IptvSourceForm | null>(source ? sourceToForm(source) : null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const activeTab: SettingsTab = source ? tab : "player";

  useEffect(() => {
    setForm(source ? sourceToForm(source) : null);
    setError(null);
  }, [source]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => event.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const update = (patch: Partial<PlayerSettings>) => onSettingsChange({ ...settings, ...patch });

  const save = async () => {
    if (!source || !form) return;
    setSaving(true);
    setError(null);
    try {
      await onSaveSource(source.id, form);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setSaving(false);
    }
  };

  const shell =
    variant === "overlay"
      ? "absolute inset-0 z-20 flex items-stretch justify-end bg-black/50"
      : "fixed inset-0 z-[120] flex items-center justify-center bg-black/70 p-4";
  const card =
    variant === "overlay"
      ? "h-full w-full max-w-md overflow-y-auto border-l border-white/10 bg-cv-panel p-4 text-cv-text"
      : "max-h-[90vh] w-full max-w-xl overflow-y-auto rounded-xl border border-white/10 bg-cv-panel p-5 text-cv-text";

  return (
    <div
      className={shell}
      data-player-settings
      onClick={(event) => event.target === event.currentTarget && onClose()}
      onMouseMove={(event) => event.stopPropagation()}
    >
      <div className={card} role="dialog" aria-modal="true" aria-label="Live TV settings">
        <div className="mb-3 flex items-center justify-between">
          <h3 className="flex items-center gap-2 text-sm font-bold">
            <Settings2 size={15} className="text-cv-accent" /> Live TV settings
          </h3>
          <button onClick={onClose} className="rounded-md p-1.5 hover:bg-white/10" aria-label="Close settings">
            <X size={15} />
          </button>
        </div>

        <div className="mb-4 flex gap-1" role="tablist">
          <button role="tab" aria-selected={activeTab === "player"} onClick={() => onTabChange("player")} className={`cv-btn text-xs ${activeTab === "player" ? "cv-btn-primary" : "cv-btn-secondary"}`}>
            <SlidersHorizontal size={12} /> Player
          </button>
          {source && (
            <button role="tab" aria-selected={activeTab === "source"} onClick={() => onTabChange("source")} className={`cv-btn text-xs ${activeTab === "source" ? "cv-btn-primary" : "cv-btn-secondary"}`}>
              <Tv size={12} /> Source: {loginTypeInfo(source.source_kind).label}
            </button>
          )}
        </div>

        {activeTab === "player" ? (
          <div className="space-y-4 text-xs">
            <div>
              <span className="section-label">Latency</span>
              <div className="grid gap-1.5">
                {LATENCY_OPTIONS.map((option) => (
                  <label key={option.value} className={`flex cursor-pointer items-start gap-2 rounded-lg p-2 ${settings.latency === option.value ? "bg-cv-accent/15 ring-1 ring-cv-accent/50" : "bg-white/[0.03] hover:bg-white/[0.06]"}`}>
                    <input type="radio" name="iptv-latency" className="mt-0.5" checked={settings.latency === option.value} onChange={() => update({ latency: option.value })} />
                    <span>
                      <span className="block font-semibold">{option.label}</span>
                      <span className="text-[11px] text-cv-subtext">{option.hint}</span>
                    </span>
                  </label>
                ))}
              </div>
            </div>
            <div>
              <label htmlFor="iptv-buffer" className="section-label">Buffer: {settings.bufferSeconds} seconds</label>
              <input id="iptv-buffer" type="range" min={10} max={120} step={5} value={settings.bufferSeconds} onChange={(e) => update({ bufferSeconds: Number(e.target.value) })} className="w-full accent-[var(--cv-accent)]" />
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label htmlFor="iptv-quality" className="section-label">Quality</label>
                <select id="iptv-quality" value={settings.quality} onChange={(e) => update({ quality: e.target.value as PlayerSettings["quality"] })} className="cv-input">
                  {QUALITY_OPTIONS.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
                </select>
              </div>
              <div>
                <label htmlFor="iptv-fit" className="section-label">Picture</label>
                <select id="iptv-fit" value={settings.videoFit} onChange={(e) => update({ videoFit: e.target.value as PlayerSettings["videoFit"] })} className="cv-input">
                  {FIT_OPTIONS.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
                </select>
              </div>
              <div>
                <label htmlFor="iptv-retries" className="section-label">Reconnect attempts</label>
                <input id="iptv-retries" type="number" min={0} max={10} value={settings.reconnectAttempts} onChange={(e) => update({ reconnectAttempts: Number(e.target.value) })} className="cv-input" />
              </div>
              <label className="mt-5 flex items-center gap-2">
                <input type="checkbox" checked={settings.autoplay} onChange={(e) => update({ autoplay: e.target.checked })} />
                Start playing when a channel opens
              </label>
            </div>
            {channelHeaders && (channelHeaders.userAgent || channelHeaders.referrer) && (
              <div className="rounded-lg bg-white/[0.03] p-2 text-[11px] text-cv-subtext">
                <div className="mb-1 font-bold uppercase tracking-wider">This channel is sent with</div>
                {channelHeaders.userAgent && <div className="truncate font-mono">User-Agent: {channelHeaders.userAgent}</div>}
                {channelHeaders.referrer && <div className="truncate font-mono">Referer: {channelHeaders.referrer}</div>}
              </div>
            )}
            <button onClick={() => onSettingsChange({ ...DEFAULT_PLAYER_SETTINGS })} className="cv-btn cv-btn-secondary text-xs">
              Reset to defaults
            </button>
          </div>
        ) : (
          form &&
          source && (
            <div className="space-y-3 text-xs">
              <IptvSourceFields form={form} onChange={setForm} hasSavedSecret={source.has_secret} idPrefix="iptv-edit" />
              {error && <p className="text-xs text-red-400">{error}</p>}
              <div className="flex items-center gap-2">
                <button onClick={() => void save()} disabled={saving} className="cv-btn cv-btn-primary text-xs">
                  {saving ? <Loader2 size={12} className="animate-spin" /> : <Save size={12} />} Save source
                </button>
                <span className="text-[11px] text-cv-subtext">Headers apply on the next reconnect; a new server or login on the next sync.</span>
              </div>
            </div>
          )
        )}
      </div>
    </div>
  );
}
