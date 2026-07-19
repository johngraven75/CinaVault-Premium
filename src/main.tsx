import React from "react";
import ReactDOM from "react-dom/client";
import "./styles/index.css";
import "./styles/cyber-hud.css";
import "./styles/metadata-actions.css";
import "./styles/kodi-skin.css";

const rootElement = document.getElementById("root");
if (!rootElement) {
  throw new Error("CinaVault root element was not found.");
}

const root = ReactDOM.createRoot(rootElement);

function StartupViewport(): React.JSX.Element {
  return (
    <div