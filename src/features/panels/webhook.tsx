// Webhooks: target URLs, which events to send, and a test send.
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Send } from "lucide-react";

import { WEBHOOK_EVENT_GROUPS, parseWebhookUrls } from "../serverAdminLogic.ts";
import type { FeaturePanel } from "./types";

type WebhookConfig = { urls?: string[]; events?: string[] };

const WebhookPanel: FeaturePanel<WebhookConfig> = ({ config, setConfig }) => {
  const urls = Array.isArray(config.urls) ? config.urls : [];
  const events = Array.isArray(config.events) ? config.events : [];
  const [text, setText] = useState(urls.join("\n"));
  const [message, setMessage] = useState<string | null>(null);

  const saveUrls = async () => {
    const parsed = parseWebhookUrls(text);
    setText(parsed.urls.join("\n"));
    try {
      await setConfig({ urls: parsed.urls });
      setMessage(parsed.invalid.length ? `Ignored: ${parsed.invalid.join(", ")}` : "Saved");
    } catch (error) {
      setMessage(String(error));
    }
  };

  const toggleEvent = (event: string) => {
    const next = events.includes(event) ? events.filter((value) => value !== event) : [...events, event];
    void setConfig({ events: next });
  };

  const test = async () => {
    const targets = parseWebhookUrls(text).urls;
    if (targets.length === 0) return setMessage("Add a URL first");
    const results = await Promise.all(
      targets.map((url) =>
        invoke<number>("webhook_test", { url })
          .then((status) => `${url}: HTTP ${status}`)
          .catch((error) => `${url}: ${error}`),
      ),
    );
    setMessage(results.join(" · "));
  };

  return (
    <div className="space-y-2">
      <label className="block space-y-1">
        <span className="text-[11px] text-cv-subtext">URLs (one per line)</span>
        <textarea
          className="cv-input min-h-[64px] w-full text-xs"
          value={text}
          placeholder="https://example.com/hooks/cinavault"
          onChange={(event) => setText(event.target.value)}
          onBlur={() => void saveUrls()}
        />
      </label>
      <div className="space-y-1">
        <span className="text-[11px] text-cv-subtext">Events (none ticked sends everything)</span>
        <div className="flex flex-wrap gap-x-3 gap-y-1">
          {WEBHOOK_EVENT_GROUPS.map((event) => (
            <label key={event} className="flex items-center gap-1 text-xs">
              <input type="checkbox" checked={events.includes(event)} onChange={() => toggleEvent(event)} />
              {event}
            </label>
          ))}
        </div>
      </div>
      <button type="button" className="cv-btn cv-btn-secondary text-xs" onClick={() => void test()}>
        <Send size={12} /> Send test
      </button>
      {message && <div className="break-all text-[11px] text-cv-subtext" role="status">{message}</div>}
    </div>
  );
};

export default WebhookPanel;
