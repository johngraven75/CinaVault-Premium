// CinaVault Premium — keeps the agent's photo portrait in app data
// (agent_avatar.rs). The photo is re-encoded there and never bundled.
import { invoke } from "@tauri-apps/api/core";

import { base64FromDataUrl } from "./holoAgent";
import { portraitImageError, type PortraitCalibration } from "./portraitAvatar";

export interface AgentAvatar {
  /** The stored photo as a data URL. */
  dataUrl: string;
  calibration: PortraitCalibration;
}

export function getAgentAvatar(): Promise<AgentAvatar | null> {
  return invoke<AgentAvatar | null>("agent_avatar_get");
}

/** Saves a photo (a data URL read from the picked file) with its markers. */
export function saveAgentAvatar(photoDataUrl: string, calibration: PortraitCalibration): Promise<AgentAvatar> {
  return invoke<AgentAvatar>("agent_avatar_save", { photo: base64FromDataUrl(photoDataUrl), calibration });
}

export function setAgentAvatarCalibration(calibration: PortraitCalibration): Promise<AgentAvatar> {
  return invoke<AgentAvatar>("agent_avatar_set_calibration", { calibration });
}

export function clearAgentAvatar(): Promise<void> {
  return invoke<void>("agent_avatar_clear");
}

/** Reads a picked photo as a data URL, after checking its type and size. */
export function readPortraitFile(file: File): Promise<string> {
  const problem = portraitImageError(file.type, file.size);
  if (problem) return Promise.reject(new Error(problem));
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(new Error("That photo could not be read."));
    reader.readAsDataURL(file);
  });
}
