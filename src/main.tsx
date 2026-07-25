import React from "react";
import ReactDOM from "react-dom/client";
import "./styles/index.css";
import "./styles/cyber-hud.css";
import "./styles/experience-shell.css";
import "./styles/build170-library.css";
import "./styles/metadata-actions.css";
import "./styles/kodi-skin.css";

const rootElement = document.getElementById("root");
if (!rootElement) {
  throw new Error("CinaVault root element was not found.");
}

const root = ReactDOM.createRoot(rootElement);

function StartupViewport(): React.JSX.Element {
  return (
    <div className="min-h-screen bg-[#030813] text-white flex items-center justify-center">
      <div className="w-[min(520px,88vw)] rounded-3xl border border-white/10 bg-[#071321]/95 p-8 shadow-2xl">
        <div className="text-xs font-bold uppercase tracking-[0.34em] text-cyan-300">
          CinaVault Premium
        </div>
        <h1 className="mt-3 text-3xl font-black">Starting media center</h1>
        <p className="mt-3 text-sm text-slate-300">
          Opening the interface now. Services continue loading in the background.
        </p>
        <div className="mt-6 h-2 overflow-hidden rounded-full bg-white/10">
          <div className="h-full w-2/3 animate-pulse rounded-full bg-cyan-400" />
        </div>
      </div>
    </div>
  );
}

root.render(<StartupViewport />);

void import("./App")
  .then(({ default: App }) => {
    root.render(<App />);
    document.getElementById("splash")?.remove();
  })
  .catch((error: unknown) => {
    console.error("CinaVault application shell failed to load:", error);
    root.render(
      <div className="min-h-screen bg-[#030813] text-white flex items-center justify-center p-8">
        <div className="max-w-xl rounded-3xl border border-red-400/30 bg-red-950/30 p-8">
          <h1 className="text-2xl font-black">CinaVault could not start</h1>
          <p className="mt-3 text-sm text-red-100">
            Restart the application. The startup error was recorded in the application log.
          </p>
        </div>
      </div>,
    );
  });
