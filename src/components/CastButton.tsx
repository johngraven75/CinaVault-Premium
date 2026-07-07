import React, { useState } from "react";

export default function CastButton() {
  const [open, setOpen] = useState(false);

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
            />
          </label>
          <button type="button" data-testid="cinavault-cast-start">
            Start Casting
          </button>
        </div>
      )}
    </section>
  );
}
