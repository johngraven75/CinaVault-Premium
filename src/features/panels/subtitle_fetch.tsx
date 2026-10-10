// Auto Subtitle Download: languages, the OpenSubtitles key, the last
// automatic run, and "Find subtitles" for a single title.
import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import type { FeaturePanel } from "./types";
import { describeSubtitleRun, formatLanguageList, parseLanguageList, type SubtitleRun } from "../library/libraryAutomation.ts";

interface SubtitleConfig {
  languages?: string[];
  hearingImpaired?: boolean;
}

interface SubtitleStatus {
  keyConfigured: boolean;
  languages: string[];
  hearingImpaired: boolean;
  lastRun: SubtitleRun | null;
}

interface FindReport {
  title: string;
  downloaded: { language: string; path: string }[];
  alreadyPresent: string[];
  notFound: string[];
  errors: string[];
}

const SubtitleFetchPanel: FeaturePanel<SubtitleConfig> = ({ config, setConfig }) => {
  const [status, setStatus] = useState<SubtitleStatus | null>(null);
  const [languages, setLanguages] = useState(formatLanguageList(config.languages));
  const [key, setKey] = useState("");
  const [query, setQuery] = useState("");
  const [matches, setMatches] = useState<{ id: number; title: string }[]>([]);
  const [message, setMessage] = useState("");

  const refresh = useCallback(() => {
    invoke<SubtitleStatus>("subtitles_status")
      .then((next) => {
        setStatus(next);
        if (!config.languages?.length) setLanguages(formatLanguageList(next.languages));
      })
      .catch((error) => setMessage(String(error)));
  }, [config.languages]);
  useEffect(refresh, [refresh]);

  const saveLanguages = async () => {
    const parsed = parseLanguageList(languages);
    setLanguages(formatLanguageList(parsed));
    await setConfig({ languages: parsed.length ? parsed : ["en"] });
  };

  const saveKey = async () => {
    try {
      await invoke("set_api_key", { provider: "opensubtitles", apiKey: key.trim() });
      setKey("");
      setMessage(key.trim() ? "API key saved to the system credential store." : "API key removed.");
      refresh();
    } catch (error) {
      setMessage(`Could not save the key: ${error}`);
    }
  };

  const search = async () => {
    const items = await invoke<{ id: number; title: string; media_type: string }[]>("search_media", { query: query.trim() });
    setMatches(items.filter((item) => item.media_type !== "music").slice(0, 6));
  };

  const find = async (id: number) => {
    setMessage("Searching OpenSubtitles...");
    try {
      const report = await invoke<FindReport>("subtitles_find", { id });
      const parts = [
        report.downloaded.length ? `saved ${report.downloaded.map((d) => d.language).join(", ")}` : "",
        report.alreadyPresent.length ? `already had ${report.alreadyPresent.join(", ")}` : "",
        report.notFound.length ? `none found for ${report.notFound.join(", ")}` : "",
        report.errors.length ? report.errors.join("; ") : "",
      ].filter(Boolean);
      setMessage(`${report.title}: ${parts.join("; ") || "nothing to do"}`);
    } catch (error) {
      setMessage(String(error));
    }
  };

  return (
    <div className="space-y-2 text-xs">
      <label className="block">
        <span className="text-cv-subtext">Languages (ISO codes, first is preferred)</span>
        <input
          className="cv-input w-full text-xs mt-1"
          value={languages}
          placeholder="en, fr, pt-br"
          onChange={(event) => setLanguages(event.target.value)}
          onBlur={() => void saveLanguages()}
        />
      </label>
      <label className="flex items-center gap-2">
        <input
          type="checkbox"
          checked={Boolean(config.hearingImpaired)}
          onChange={(event) => void setConfig({ hearingImpaired: event.target.checked })}
        />
        <span>Prefer hearing-impaired (SDH) subtitles</span>
      </label>
      <div>
        <div className="text-cv-subtext">
          OpenSubtitles API key:{" "}
          {status ? (status.keyConfigured ? "stored" : "not set; automatic downloads are skipped") : "checking..."}
        </div>
        <div className="flex gap-1 mt-1">
          <input
            type="password"
            className="cv-input flex-1 text-xs"
            value={key}
            placeholder="Key from opensubtitles.com/consumers"
            autoComplete="off"
            onChange={(event) => setKey(event.target.value)}
          />
          <button type="button" className="cv-btn cv-btn-secondary text-[10px] px-2" onClick={() => void saveKey()}>
            {key.trim() ? "Save key" : "Remove key"}
          </button>
        </div>
      </div>
      <div className="text-cv-subtext">Last automatic run: {describeSubtitleRun(status?.lastRun)}</div>
      <div>
        <div className="text-cv-subtext">Find subtitles for one title</div>
        <div className="flex gap-1 mt-1">
          <input
            className="cv-input flex-1 text-xs"
            value={query}
            placeholder="Title"
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => event.key === "Enter" && void search()}
          />
          <button type="button" className="cv-btn cv-btn-secondary text-[10px] px-2" disabled={!query.trim()} onClick={() => void search()}>
            Search
          </button>
        </div>
        {matches.map((item) => (
          <div key={item.id} className="flex items-center justify-between gap-2 mt-1">
            <span className="truncate">{item.title}</span>
            <button type="button" className="cv-btn cv-btn-secondary text-[10px] px-2 py-0.5" onClick={() => void find(item.id)}>
              Find subtitles
            </button>
          </div>
        ))}
      </div>
      {message && <div className="text-cv-subtext" role="status">{message}</div>}
    </div>
  );
};

export default SubtitleFetchPanel;
