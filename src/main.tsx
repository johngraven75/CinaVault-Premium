import React from "react";
import ReactDOM from "react-dom/client";
import "./data/pluginAdapterInitialize";
import App from "./App";
import "./styles/index.css";
import "./styles/cyber-hud.css";
import "./styles/metadata-actions.css";
import "./styles/kodi-skin.css";

// ── Logo-based loading screen (replaces old text splash + brand-splash) ──
const splash = document.getElementById("splash");
const splashBar = document.getElementById("splash-bar") as HTMLDivElement;
const splashStatus = document.getElementById("splash-status") as HTMLDivElement;

const stages = [
  { pct: 15, text: "Loading media engine..." },
  { pct: 35, text: "Initializing Rust backend..." },
  { pct: 55, text: "Mounting CinaVault interface..." },
  { pct: 75, text: "Preparing media library systems..." },
  { pct: 90, text: "Activating AI features..." },
  { pct: 100, text: "Ready." },
];

let stageIdx = 0;

function advanceSplash(): void {
  if (stageIdx < stages.length) {
    const stage = stages[stageIdx];
    if (splashBar) splashBar.style.width = `${stage.pct}%`;
    if (splashStatus) splashStatus.textContent = stage.text;
    stageIdx += 1;
    window.setTimeout(advanceSplash, 300 + Math.random() * 200);
    return;
  }
  // Fade out and remove the loading screen
  window.setTimeout(() => {
    if (splash) splash.classList.add("hidden");
    window.setTimeout(() => splash?.remove(), 900);
  }, 260);
}

advanceSplash();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
