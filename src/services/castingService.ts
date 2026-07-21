export type CastingDeviceType = "chromecast" | "airplay" | "smartview" | "dlna";

export interface CastingDevice {
  id: string;
  name: string;
  address?: string;
  type: CastingDeviceType;
  connected: boolean;
}

export interface CastingSession {
  device: CastingDevice;
  mediaUrl: string;
  title?: string;
}

export async function discoverCastingDevices(): Promise<CastingDevice[]> {
  // Discovery adapters are intentionally isolated so native Tauri discovery
  // can be added without changing the UI layer.
  return [];
}

export async function connectCastingDevice(device: CastingDevice): Promise<void> {
  void device;
}

export async function disconnectCastingDevice(device: CastingDevice): Promise<void> {
  void device;
}

export async function startCasting(session: CastingSession): Promise<string> {
  return `Casting started on ${session.device.name}`;
}
