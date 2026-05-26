import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles/index.css";

// Splash screen animation
const splash = document.getElementById("splash");
const splashBar = document.getElementById("splash-bar") as HTMLDivElement;
const splashStatus = document.getElementById("splash-status") as HTMLDivElement;

const stages = [
  { pct: 15, text: "Loading core engine..." },
  { pct: 35, text: "Initializing Rust backend..." },
  { pct: 55, text: "Mounting premium UI..." },
  { pct: 75, text: "Preparing media systems..." },
  { pct: 90, text: "Activating visual effects..." },
  { pct: 100, text: "Ready." },
];

let stageIdx = 0;
const advanceSplash = () => {
  if (stageIdx < stages.length) {
    const s = stages[stageIdx];
    if (splashBar) splashBar.style.width = `${s.pct}%`;
    if (splashStatus) splashStatus.textContent = s.text;
    stageIdx++;
    setTimeout(advanceSplash, 300 + Math.random() * 200);
  } else {
    setTimeout(() => {
      if (splash) splash.classList.add("hidden");
      setTimeout(() => splash?.remove(), 760);
    }, 260);
  }
};
advanceSplash();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
