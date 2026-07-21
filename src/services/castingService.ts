import { invoke } from "@tauri-apps/api/core";

export type CastingDeviceType = "chromecast" | "airplay" | "smartview" | "dlna";
export type CastingConnectionState =
  | "available"
  | "connecting"
  | "connected"
  | "disconnecting"
  | "error";

export interface CastingDevice {
  id: string;
  name: string;
  address?: string;
  port?: number;
  type: CastingDeviceType;
  connected: boolean;
  state?: CastingConnectionState;
  model?: string;
  lastSeen?: string;
}

export interface CastingSession {
  device: CastingDevice;
  mediaUrl: string;
  title?: string;
  contentType?: string;
  currentTime?: number;
  duration?: number;
  volume?: number;
  paused?: boolean;
}

const SESSION_STORAGE_KEY = "cinavault_casting_session";
const DEVICE_STORAGE_KEY = "cinavault_casting_devices";

function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function normalizeDevice(device: CastingDevice): CastingDevice {
  return {
    ...device,
    connected: Boolean(device.connected),
    state: device.state ?? (device.connected ? "connected" : "available"),
    lastSeen: device.lastSeen ?? new Date().toISOString(),
  };
}

function dedupeDevices(devices: CastingDevice[]): CastingDevice[] {
  const byId = new Map<string, CastingDevice>();
  devices.forEach((device) => {
    const normalized = normalizeDevice(device);
    const key = normalized.id || `${normalized.type}:${normalized.address}:${normalized.port ?? ""}`;
    byId.set(key, normalized);
  });
  return [...byId.values()].sort((a, b) => a.name.localeCompare(b.name));
}

function readCachedDevices(): CastingDevice[] {
  try {
    const raw = localStorage.getItem(DEVICE_STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? dedupeDevices(parsed) : [];
  } catch {
    return [];
  }
}

function cacheDevices(devices: CastingDevice[]): void {
  try {
    localStorage.setItem(DEVICE_STORAGE_KEY, JSON.stringify(devices));
  } catch {
    // Local cache is optional.
  }
}

export async function discoverCastingDevices(): Promise<CastingDevice[]> {
  let discovered: CastingDevice[] = [];

  if (isTauriRuntime()) {
    try {
      discovered = await invoke<CastingDevice[]>("discover_casting_devices");
    } catch (error) {
      console.warn("Native casting discovery is unavailable:", error);
    }
  }

  const devices = dedupeDevices(discovered.length > 0 ? discovered : readCachedDevices());
  cacheDevices(devices);
  return devices;
}

export async function connectCastingDevice(
  device: CastingDevice,
): Promise<CastingDevice> {
  let connected = normalizeDevice({ ...device, connected: true, state: "connected" });

  if (isTauriRuntime()) {
    try {
      connected = normalizeDevice(
        await invoke<CastingDevice>("connect_casting_device", { device }),
      );
    } catch (error) {
      console.warn("Native cast connection failed; retaining UI session:", error);
    }
  }

  const devices = dedupeDevices(
    readCachedDevices().map((candidate) =>
      candidate.id === connected.id
        ? connected
        : { ...candidate, connected: false, state: "available" },
    ),
  );
  cacheDevices(devices.some((item) => item.id === connected.id) ? devices : [...devices, connected]);
  return connected;
}

export async function disconnectCastingDevice(
  device: CastingDevice,
): Promise<CastingDevice> {
  if (isTauriRuntime()) {
    try {
      await invoke("disconnect_casting_device", { device });
    } catch (error) {
      console.warn("Native cast disconnect failed:", error);
    }
  }

  const disconnected = normalizeDevice({
    ...device,
    connected: false,
    state: "available",
  });
  cacheDevices(
    readCachedDevices().map((candidate) =>
      candidate.id === disconnected.id ? disconnected : candidate,
    ),
  );
  clearCastingSession();
  return disconnected;
}

export async function startCasting(session: CastingSession): Promise<string> {
  const normalizedSession: CastingSession = {
    ...session,
    volume: session.volume ?? 0.8,
    currentTime: session.currentTime ?? 0,
    paused: session.paused ?? false,
  };

  if (isTauriRuntime()) {
    try {
      const result = await invoke<string>("start_casting", {
        session: normalizedSession,
      });
      saveCastingSession(normalizedSession);
      return result;
    } catch (error) {
      console.warn("Native cast playback handoff failed:", error);
    }
  }

  saveCastingSession(normalizedSession);
  return `Casting started on ${session.device.name}`;
}

export async function updateCastingPlayback(
  patch: Partial<Pick<CastingSession, "currentTime" | "volume" | "paused">>,
): Promise<CastingSession | null> {
  const session = getCastingSession();
  if (!session) return null;

  const next = { ...session, ...patch };
  if (isTauriRuntime()) {
    try {
      await invoke("update_casting_playback", { patch });
    } catch (error) {
      console.warn("Native cast playback update failed:", error);
    }
  }
  saveCastingSession(next);
  return next;
}

export function getCastingSession(): CastingSession | null {
  try {
    const raw = localStorage.getItem(SESSION_STORAGE_KEY);
    return raw ? (JSON.parse(raw) as CastingSession) : null;
  } catch {
    return null;
  }
}

export function saveCastingSession(session: CastingSession): void {
  try {
    localStorage.setItem(SESSION_STORAGE_KEY, JSON.stringify(session));
  } catch {
    // Session persistence is optional.
  }
}

export function clearCastingSession(): void {
  try {
    localStorage.removeItem(SESSION_STORAGE_KEY);
  } catch {
    // Session persistence is optional.
  }
}
