// Live TV source logins: which fields each provider type needs, and the
// arguments for the add_iptv_source / update_iptv_source commands.
import { normalizeXtreamServerUrl } from "./xtreamProfile.ts";

export type IptvLoginType = "xtream" | "m3u" | "stalker";

export interface LoginTypeInfo {
  id: IptvLoginType;
  label: string;
  hint: string;
  serverLabel: string;
  serverPlaceholder: string;
  /** Xtream only. */
  needsUsername: boolean;
  /** Password (Xtream) or MAC address (Stalker). None for M3U. */
  secretLabel: string | null;
  secretPlaceholder: string;
}

export const LOGIN_TYPES: ReadonlyArray<LoginTypeInfo> = [
  {
    id: "xtream",
    label: "Xtream Codes",
    hint: "Server address, username and password from your provider.",
    serverLabel: "Server URL",
    serverPlaceholder: "https://provider.com:8080",
    needsUsername: true,
    secretLabel: "Password",
    secretPlaceholder: "",
  },
  {
    id: "m3u",
    label: "M3U playlist",
    hint: "A playlist link. Any login or token stays inside the link.",
    serverLabel: "Playlist URL (M3U / M3U8)",
    serverPlaceholder: "https://provider.com/get.php?username=…&type=m3u_plus",
    needsUsername: false,
    secretLabel: null,
    secretPlaceholder: "",
  },
  {
    id: "stalker",
    label: "Stalker portal (MAC)",
    hint: "A MAG / Ministra portal that signs in with a set-top box MAC address.",
    serverLabel: "Portal URL",
    serverPlaceholder: "https://portal.provider.com/c/",
    needsUsername: false,
    secretLabel: "MAC address",
    secretPlaceholder: "00:1A:79:XX:XX:XX",
  },
];

export function loginTypeInfo(kind: string | undefined): LoginTypeInfo {
  return LOGIN_TYPES.find((type) => type.id === kind) ?? LOGIN_TYPES[0];
}

/** Agents providers commonly whitelist. "" means the app's own. */
export const USER_AGENT_PRESETS: ReadonlyArray<{ label: string; value: string }> = [
  { label: "CinaVault (default)", value: "" },
  { label: "VLC", value: "VLC/3.0.21 LibVLC/3.0.21" },
  { label: "Kodi", value: "Kodi/21.1 (Windows NT 10.0; Win64; x64) App_Bitness/64 Version/21.1" },
  { label: "TiviMate", value: "TiviMate/5.1.6 (Android 12)" },
  { label: "IPTV Smarters", value: "IPTVSmartersPro" },
  { label: "MAG box", value: "Mozilla/5.0 (QtEmbedded; U; Linux; C) AppleWebKit/533.3 (KHTML, like Gecko) MAG200 stbapp ver: 2 rev: 250 Safari/533.3" },
  { label: "Chrome (Windows)", value: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36" },
];

export interface IptvSourceForm {
  kind: IptvLoginType;
  name: string;
  serverUrl: string;
  username: string;
  /** Password or MAC. Left empty when editing keeps the saved one. */
  secret: string;
  userAgent: string;
  referrer: string;
}

export const EMPTY_SOURCE_FORM: Readonly<IptvSourceForm> = Object.freeze({
  kind: "xtream",
  name: "",
  serverUrl: "",
  username: "",
  secret: "",
  userAgent: "",
  referrer: "",
});

export interface IptvSourceArgs extends Record<string, unknown> {
  source: {
    kind: IptvLoginType;
    name: string;
    serverUrl: string;
    username: string | null;
    secret: string | null;
    userAgent: string | null;
    referrer: string | null;
  };
}

const MAC_PATTERN = /^[0-9a-f]{2}([:-]?[0-9a-f]{2}){5}$/i;

function header(label: string, value: string): string | null {
  const trimmed = value.trim();
  if (!trimmed) return null;
  // Same rule as the backend: a line break would split the HTTP request.
  if (/[\u0000-\u001f\u007f]/.test(trimmed)) throw new Error(`${label} cannot contain line breaks.`);
  if (trimmed.length > 512) throw new Error(`${label} is longer than 512 characters.`);
  return trimmed;
}

/** Checks a source form before it goes to the backend (which checks again). */
export function buildIptvSourceArgs(form: IptvSourceForm, { editing = false } = {}): IptvSourceArgs {
  const username = form.username.trim();
  const secret = form.secret.trim();
  // A public playlist is just a URL with no login. An Xtream login with neither
  // a username nor a password can't work, so treat that as an M3U playlist —
  // this lets a bare public M3U/M3U8 URL save without the user first switching
  // the login type away from the default.
  const kind: IptvLoginType =
    form.kind === "xtream" && !username && !secret ? "m3u" : form.kind;
  const info = loginTypeInfo(kind);
  const raw = form.serverUrl.trim();
  if (!raw) throw new Error(`${info.serverLabel} is required.`);
  let serverUrl = raw;
  if (kind === "xtream") {
    serverUrl = normalizeXtreamServerUrl(raw);
  } else if (!/^https?:\/\/[^/\s]+/i.test(raw)) {
    throw new Error(`Enter the full ${info.serverLabel.toLowerCase()} starting with http:// or https://.`);
  }
  if (info.needsUsername && !username) throw new Error("Username is required.");
  if (info.secretLabel && !secret && !editing) throw new Error(`${info.secretLabel} is required.`);
  if (kind === "stalker" && secret && !MAC_PATTERN.test(secret)) {
    throw new Error("Enter the MAC address as 00:1A:79:XX:XX:XX.");
  }
  return {
    source: {
      kind,
      name: form.name.trim(),
      serverUrl,
      username: info.needsUsername ? username : null,
      secret: info.secretLabel && secret ? secret : null,
      userAgent: header("User-Agent", form.userAgent),
      referrer: header("Referer", form.referrer),
    },
  };
}
