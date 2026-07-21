import { useEffect, useState } from "react";
import {
  CastingDevice,
  discoverCastingDevices,
} from "../../services/castingService";

export default function CastingTab() {
  const [devices, setDevices] = useState<CastingDevice[]>([]);
  const [scanning, setScanning] = useState(false);

  const scan = async () => {
    setScanning(true);
    try {
      setDevices(await discoverCastingDevices());
    } finally {
      setScanning(false);
    }
  };

  useEffect(() => {
    void scan();
  }, []);

  return (
    <section className="space-y-6" data-testid="cinavault-casting-tab">
      <div>
        <h2 className="text-3xl font-black">Casting</h2>
        <p className="text-cv-subtext">
          Discover and control Chromecast, AirPlay, Smart View, and DLNA devices.
        </p>
      </div>

      <button type="button" onClick={() => void scan()} disabled={scanning}>
        {scanning ? "Scanning…" : "Scan Devices"}
      </button>

      <div className="grid gap-3">
        {devices.length === 0 ? (
          <div>No casting devices discovered yet.</div>
        ) : (
          devices.map((device) => (
            <div key={device.id} className="rounded-xl border p-4">
              <strong>{device.name}</strong>
              <div>{device.type}</div>
            </div>
          ))
        )}
      </div>
    </section>
  );
}
