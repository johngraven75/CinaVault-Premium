import { useEffect, useMemo, useState } from "react";
import { Cast, RefreshCw, X, Wifi } from "lucide-react";
import { useAppStore } from "../../store/appStore";
import {
  castMediaToDevice,
  discoverCastDevices,
  type CastDevice,
  type CastProtocol,
} from "../../services/castDevices";

const PROTOCOL_LABELS: Record<CastProtocol, string> = {
  google_cast: "Google Cast",
  samsung_smart_view: "Samsung Smart View",
  airplay: "Apple AirPlay",
};

interface CastingTabProps {
  onClose: () => void;
}

export default function CastingTab({ onClose }: CastingTabProps) {
  const { selectedMedia, serverUrl, addStatusMessage } = useAppStore();
  const [devices, setDevices] = useState<CastDevice[]>([]);
  const [selectedDeviceId, setSelectedDeviceId] = useState("");
  const [discovering, setDiscovering] = useState(false);
  const [casting, setCasting] = useState(false);
  const [error, setError] = useState("");

  const mediaUrl = useMemo(() => {
    const path = selectedMedia?.file_path || "";
    if (/^https?:\/\//i.test(path)) return path;
    if (serverUrl && selectedMedia?.id != null) {
      return `${serverUrl.replace(/\/$/, "")}/media/${selectedMedia.id}`;
    }
    return path;
  }, [selectedMedia, serverUrl]);

  const selectedDevice = devices.find((device) => device.id === selectedDeviceId);

  const refreshDevices = async () => {
    setDiscovering(true);
    setError("");
    try {
      const found = await discoverCastDevices();
      setDevices(found);
      if (found.length && !found.some((device) => device.id === selectedDeviceId)) {
        setSelectedDeviceId(found[0].id);
      }
      addStatusMessage(`Casting discovery found ${found.length} nearby device${found.length === 1 ? "" : "s"}`);
    } catch (discoveryError) {
      const message = String(discoveryError);
      setError(message);
      addStatusMessage(`Casting discovery failed: ${message}`);
    } finally {
      setDiscovering(false);
    }
  };

  useEffect(() => {
    void refreshDevices();
  }, []);

  const startCasting = async () => {
    if (!selectedDevice || !mediaUrl || casting) return;
    setCasting(true);
    setError("");
    try {
      const result = await castMediaToDevice({
        device: selectedDevice,
        url: mediaUrl,
        title: selectedMedia?.title,
        posterUrl: selectedMedia?.poster_path || undefined,
      });
      addStatusMessage(`${result}: ${selectedMedia?.title || mediaUrl}`);
    } catch (castError) {
      const message = String(castError);
      setError(message);
      addStatusMessage(`Casting failed: ${message}`);
    } finally {
      setCasting(false);
    }
  };

  return (
    <div className="fixed inset-4 z-[80] overflow-hidden rounded-[32px] border border-white/15 bg-[#06101d]/95 shadow-[0_35px_120px_rgba(0,0,0,0.75)] backdrop-blur-2xl">
      <div className="flex h-full flex-col">
        <header className="flex items-center justify-between border-b border-white/10 px-7 py-5">
          <div>
            <div className="text-[10px] font-bold uppercase tracking-[0.28em] text-cyan-200">Wireless playback</div>
            <h1 className="mt-1 text-3xl font-black tracking-tight">Casting Center</h1>
            <p className="mt-1 text-sm text-cv-subtext">Choose a nearby receiver. No TV or computer IP address is required.</p>
          </div>
          <button type="button" onClick={onClose} className="grid h-11 w-11 place-items-center rounded-2xl border border-white/10 bg-white/[0.05] hover:bg-white/[0.1]" aria-label="Close Casting Center">
            <X size={19} />
          </button>
        </header>

        <div className="grid min-h-0 flex-1 grid-cols-1 gap-5 overflow-y-auto p-6 xl:grid-cols-[1.2fr_0.8fr]">
          <section className="rounded-[26px] border border-white/10 bg-black/20 p-5">
            <div className="mb-4 flex items-center justify-between gap-3">
              <div>
                <h2 className="text-xl font-bold">Nearby devices</h2>
                <p className="text-sm text-cv-subtext">Google Cast, Samsung Smart View, and Apple AirPlay receivers on this network.</p>
              </div>
              <button type="button" onClick={() => void refreshDevices()} disabled={discovering} className="flex items-center gap-2 rounded-2xl border border-cyan-200/25 bg-cyan-300/10 px-4 py-2 text-sm font-bold text-cyan-100 disabled:opacity-50">
                <RefreshCw size={16} className={discovering ? "animate-spin" : ""} />
                Refresh devices
              </button>
            </div>

            {devices.length === 0 ? (
              <div className="grid min-h-56 place-items-center rounded-3xl border border-dashed border-white/15 bg-white/[0.025] text-center">
                <div>
                  <Wifi className="mx-auto mb-3 text-cyan-200" size={32} />
                  <div className="font-bold">{discovering ? "Searching nearby receivers..." : "No receivers found"}</div>
                  <p className="mt-1 max-w-md text-sm text-cv-subtext">Make sure the TV or receiver is powered on and connected to the same local network.</p>
                </div>
              </div>
            ) : (
              <div className="space-y-3">
                {devices.map((device) => {
                  const active = selectedDeviceId === device.id;
                  return (
                    <button key={device.id} type="button" onClick={() => setSelectedDeviceId(device.id)} className={`flex w-full items-center gap-4 rounded-3xl border p-4 text-left transition ${active ? "border-cyan-200/50 bg-cyan-300/12 shadow-[0_0_30px_rgba(34,211,238,0.12)]" : "border-white/10 bg-white/[0.035] hover:bg-white/[0.07]"}`}>
                      <span className="grid h-12 w-12 place-items-center rounded-2xl border border-white/10 bg-black/25"><Cast size={21} /></span>
                      <span className="min-w-0 flex-1">
                        <span className="block truncate text-base font-bold">{device.name}</span>
                        <span className="block text-xs uppercase tracking-[0.18em] text-cv-subtext">{PROTOCOL_LABELS[device.protocol]}</span>
                      </span>
                      <span className="text-xs text-cv-subtext">Nearby</span>
                    </button>
                  );
                })}
              </div>
            )}
          </section>

          <aside className="rounded-[26px] border border-white/10 bg-black/20 p-5">
            <h2 className="text-xl font-bold">Now selected</h2>
            <div className="mt-4 rounded-3xl border border-white/10 bg-white/[0.035] p-4">
              <div className="text-xs uppercase tracking-[0.2em] text-cv-subtext">Media</div>
              <div className="mt-1 font-bold">{selectedMedia?.title || "Select media from the library first"}</div>
              <div className="mt-3 break-all text-xs text-cv-subtext">{mediaUrl || "No streamable media URL is available."}</div>
            </div>
            <div className="mt-4 rounded-3xl border border-white/10 bg-white/[0.035] p-4">
              <div className="text-xs uppercase tracking-[0.2em] text-cv-subtext">Receiver</div>
              <div className="mt-1 font-bold">{selectedDevice?.name || "Choose a nearby device"}</div>
              <div className="mt-1 text-sm text-cv-subtext">{selectedDevice ? PROTOCOL_LABELS[selectedDevice.protocol] : "Google Cast / Samsung Smart View / Apple AirPlay"}</div>
            </div>
            {error && <div className="mt-4 rounded-2xl border border-rose-300/25 bg-rose-400/10 p-3 text-sm text-rose-100">{error}</div>}
            <button type="button" onClick={() => void startCasting()} disabled={!selectedDevice || !mediaUrl || casting} className="mt-5 flex w-full items-center justify-center gap-2 rounded-2xl bg-cyan-300 px-5 py-3 font-black text-slate-950 disabled:cursor-not-allowed disabled:opacity-40">
              <Cast size={18} />
              {casting ? "Connecting..." : "Cast selected media"}
            </button>
          </aside>
        </div>
      </div>
    </div>
  );
}
