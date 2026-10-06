import {
  startCasting,
  type CastingDevice,
  type CastingSession,
} from "./castingService";

export type GoogleCastMedia = {
  host: string;
  url: string;
  title?: string;
  contentType?: string;
  posterUrl?: string;
};

export type CastSession = {
  device: CastingDevice;
  mediaUrl: string;
  title?: string;
  contentType?: string;
};

export async function castToGoogleDevice(
  media: GoogleCastMedia,
): Promise<string> {
  const session: CastingSession = {
    device: {
      id: `chromecast-${media.host}`,
      name: media.title || "Google Cast Device",
      address: media.host,
      type: "chromecast",
      connected: true,
      state: "connected",
    },
    mediaUrl: media.url,
    title: media.title || "CinaVault Premium",
    contentType: media.contentType || "video/mp4",
  };

  return await startCasting(session);
}
