import React, { useState, useCallback } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { castToGoogleDevice } from "../services/googleCast";
import { useAppStore } from "../store/appStore";

function resolveCastUrl(filePath?: string | null): string {
  if (!filePath) return "";
  if (/^(https?:|data:|asset:)/i.test(filePath)) return filePath;
  try {
    return convertFileSrc(filePath);
  } catch {
    return filePath;
  }
}

export default function CastButton() {
  const { selectedMedia, addStatusMessage } = useAppStore();
  const [open, setOpen] = useState(false);
  const [host, setHost] = useState("");
  const [mediaUrl, setMediaUrl] = useState("");
  const [casting, setCasting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleStartCast = useCallback(async () => {
    const castHost = host.trim();
    const castUrl = (mediaUrl.trim() || resolveCastUrl(selectedMedia?.file_path)).trim();

    if (!castHost) {
      setError("Enter the Chromecast device IP or hostname.");
      return;
    }
    if (!castUrl) {
      setError("Select library media or enter a stream URL to cast.");
      return;
    }

    setCasting(true);
    setError(null);
    try {
      await castToGoogleDevice({
        host: castHost,
        url: castUrl,
        title: selectedMedia?.title || "CinaVault Premium",
        posterUrl: selectedMedia?.poster_path
          ? resolveCastUrl(selectedMedia.poster_path)
          : undefined,
      });
      addStatusMessage(`Casting to ${castHost}`);
      setOpen(false);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(message);
      addStatusMessage(`Cast failed: ${message}`);
    } finally {
      setCasting(false);
    }
  }, [host, mediaUrl, selectedMedia, addStatusMessage]);

  return (
    <section className="cinavault-cast-panel" aria-label="CinaVault Cast">
      <button
        type="button"
        className="cinavault-cast-button"
        data-testid="cinavault-cast-button"
        aria-label="Cast to Google Cast or Chromecast device"
        title="Cast to Google Cast / Chromecast"
        onClick={() => setOpen((value) => !value)}
      >
        📺 Cast
      </button>

      {open && (
        <div className="cinavault-cast-tab" data-testid="cinavault-cast-tab">
          <strong>Google Cast</strong>
          <p>Cast this movie or video to a Chromecast / Google Cast device.</p>
          <label>
            Device IP / Host
            <input
              data-testid="cinavault-cast-host"
              placeholder="Example: 192.168.1.50"
              value={host}
              onChange={(event) => setHost(event.target.value)}
            />
          </label>
          <label>
            Media URL (optional)
            <input
              data-testid="cinavault-cast-url"
              placeholder={
                selectedMedia?.title
                  ? `Using: ${selectedMedia.title}`
                  : "https://… or select library media"
              }
              value={mediaUrl}
              onChange={(event) => setMediaUrl(event.target.value)}
            />
          </label>
          {error && <p className="text-xs text-red-400">{error}</p>}
          <button
            type="button"
            data-testid="cinavault-cast-start"
            disabled={casting}
            onClick={() => void handleStartCast()}
          >
            {casting ? "Connecting…" : "Start Casting"}
          </button>
        </div>
      )}
    </section>
  );
}
