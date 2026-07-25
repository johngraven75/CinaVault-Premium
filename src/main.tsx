import React from "react";
import ReactDOM from "react-dom/client";
import RuntimeErrorBoundary from "./components/RuntimeErrorBoundary";
import "./styles/index.css";
import "./styles/cyber-hud.css";
import "./styles/experience-shell.css";
import "./styles/build170-library.css";
import "./styles/metadata-actions.css";
import "./styles/kodi-skin.css";
import "./styles/ui-stability.css";

const rootElement = document.getElementById("root");
if (!rootElement) {
  throw new Error("CinaVault root element was not found.");
}

const root = ReactDOM.createRoot(rootElement);
const GLOBAL_ERROR_STORAGE_KEY = "cinavault_last_global_error";

function describeUnknownError(value: unknown): string {
  if (value instanceof Error) return value.stack || value.message;
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

function recordGlobalError(kind: string, value: unknown): void {
  const record = {
    kind,
    message: describeUnknownError(value),
    occurredAt: new Date().toISOString(),
    build: "v2 Build 1.02",
  };
  console.error("CinaVault global interface error:", record);
  try {
    localStorage.setItem(GLOBAL_ERROR_STORAGE_KEY, JSON.stringify(record));
  } catch {
    // Storage failures must never create a second interface failure.
  }
}

window.addEventListener("error", (event) => {
  recordGlobalError("window.error", event.error || event.message);
});

window.addEventListener("unhandledrejection", (event) => {
  recordGlobalError("unhandledrejection", event.reason);
});

document.documentElement.dataset.cinavaultBuild = "v2-build-1-02";

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
    root.render(
      <RuntimeErrorBoundary scope="desktop-application-shell">
        <App />
      </RuntimeErrorBoundary>,
    );
    document.getElementById("splash")?.remove();
  })
  .catch((error: unknown) => {
    recordGlobalError("application-import", error);
    console.error("CinaVault application shell failed to load:", error);
    root.render(
      <div className="cv-runtime-fallback" role="alert">
        <section className="cv-runtime-fallback-card">
          <div className="cv-runtime-fallback-kicker">v2 Build 1.02 recovery</div>
          <h1>CinaVault could not load the interface</h1>
          <p>
            Restart the application. The startup error was recorded locally for
            diagnostics and no library data was removed.
          </p>
          <div className="cv-runtime-fallback-actions">
            <button type="button" onClick={() => window.location.reload()}>
              Reload interface
            </button>
          </div>
        </section>
      </div>,
    );
    document.getElementById("splash")?.remove();
  });
