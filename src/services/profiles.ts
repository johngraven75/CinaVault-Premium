// Front-end access to profiles and parental controls (user_data.rs,
// parental.rs). Anything that changes what the library shows announces it so
// the library and the shelves reload.
import { invoke } from "@tauri-apps/api/core";

import {
  PROFILE_CHANGE_EVENTS,
  type ParentalStatus,
  type Profile,
} from "../features/profileLogic.ts";

export function announceLibraryChange(reason: string): void {
  for (const name of PROFILE_CHANGE_EVENTS) {
    window.dispatchEvent(new CustomEvent(name, { detail: { reason } }));
  }
}

export const listProfiles = () => invoke<Profile[]>("profiles_list");
export const activeProfile = () => invoke<Profile>("profile_active");
export const parentalStatus = () => invoke<ParentalStatus>("parental_status");

export async function switchProfile(id: number, pin?: string): Promise<Profile> {
  const profile = await invoke<Profile>("profile_switch", { id, pin: pin || null });
  announceLibraryChange("profile-switched");
  return profile;
}

export async function saveProfile(profile: Pick<Profile, "id" | "name" | "color" | "restricted">): Promise<Profile> {
  const saved = await invoke<Profile>("profile_update", profile);
  announceLibraryChange("profile-updated");
  return saved;
}

export async function createProfile(name: string, color: string, restricted: boolean): Promise<Profile> {
  const profile = await invoke<Profile>("profile_create", { name, color, restricted });
  announceLibraryChange("profile-created");
  return profile;
}

export async function deleteProfile(id: number): Promise<void> {
  await invoke("profile_delete", { id });
  announceLibraryChange("profile-deleted");
}

export const unlockParental = (pin: string) => invoke<ParentalStatus>("parental_unlock", { pin });
export const lockParental = () => invoke<ParentalStatus>("parental_lock");
export const setParentalPin = (newPin: string, currentPin?: string) =>
  invoke<ParentalStatus>("parental_set_pin", { newPin, currentPin: currentPin || null });
export const refreshRatings = (force = false) =>
  invoke<{ checked: number; fromNfo: number; fromTmdb: number; unrated: number }>("parental_refresh_ratings", { force });
