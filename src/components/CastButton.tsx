import { useEffect, useState } from "react";
import { castToGoogleDevice } from "../services/googleCast";
import { useAppStore } from "../store/appStore";

export default function CastButton() {
  const { selectedMedia, addStatusMessage } = useAppStore();
  const [open, setOpen] = useState(false);
  const [host, setHost] = useState("");
  const [mediaUrl, setMediaUrl] = useState("");
  const [casting, setCasting] = useState(false);

  useEffect(() => {
    setMediaUrl(selectedMedia?.file_path || "");
  }, [selectedMedia?.id, selectedMedia?.file_path]);

  const startCasting = async () => {
    if (!host.trim() || !mediaUrl.trim() || casting) return;
    setCasting(true);
    try {
      const message = await castToGoogleDevice({
        host: host.trim(),
        url: mediaUrl.trim(),
        title: selectedMedia?.title,
        posterUrl: selectedMedia?.poster_path || undefined,
      });
      addStatusMessage(`${message}: ${selectedMedia?.title || mediaUrl}`);
    } catch (error) {
      addStatusMessage(`Google Cast failed: ${error}`);
    } finally {
      setCasting(false);
    }
  };

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
          <p>Cast the selected media or a reachable media URL to Chromecast.</p>
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
            Media URL
            <input
              data-testid="cinavault-cast-media-url"
              placeholder="http://CinaVault-server/media/movie.mp4"
              value={mediaUrl}
              onChange={(event) => setMediaUrl(event.target.value)}
            />
          </label>
          <button
            type="button"
            data-testid="cinavault-cast-start"
            disabled={casting || !host.trim() || !mediaUrl.trim()}
            onClick={() => void startCasting()}
          >
            {casting ? "Starting…" : "Start Casting"}
          </button>
        </div>
      )}
    </section>
  );
}
