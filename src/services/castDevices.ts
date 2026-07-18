import { invoke } from "@tauri-apps/api/core";
import { castToGoogleDevice } from "./googleCast";

export type CastProtocol = "google_cast" | "samsung_smart_view" | "airplay";

export interface CastDevice {
  id: string;
  name: string;
  protocol: CastProtocol;
  host: string;
  port: number;
  controlUrl?: string;
}

export interface CastMediaRequest {
  device: CastDevice;
  url: string;
  title?: string;
  contentType?: string;
  posterUrl?: string;
}

export async function discoverCastDevices(): Promise<CastDevice[]> {
  return invoke<CastDevice[]>("discover_cast_devices");
}

export async function castMediaToDevice(request: CastMediaRequest): Promise<string> {
  if (request.device.protocol === "google_cast") {
    return castToGoogleDevice({
      host: request.device.host,
      url: request.url,
      title: request.title,
      contentType: request.contentType,
      posterUrl: request.posterUrl,
    });
  }

  return invoke<string>("cast_media_to_device", {
    device: request.device,
    mediaUrl: request.url,
    title: request.title || "CinaVault Premium",
    contentType: request.contentType || "video/mp4",
  });
}
