// Pure logic behind the holographic AI agent: the conversation state machine,
// the procedural point-cloud head, its projection, mouth animation and the
// plain-language summary of an ai_query result. No DOM or React, so it is
// unit tested and shared by the WebGL renderer and its 2D fallback.

export type AgentMood = "idle" | "listening" | "thinking" | "speaking" | "error";

export interface AgentState {
  mood: AgentMood;
  /** Prompt currently being answered, if any. */
  pending: string | null;
}

export type AgentEvent =
  | { type: "focus" }
  | { type: "blur" }
  | { type: "submit"; prompt: string }
  | { type: "answer" }
  | { type: "fail" }
  | { type: "done" };

export const AGENT_START: AgentState = Object.freeze({ mood: "idle", pending: null });

/** Transitions between moods. Events that make no sense in a mood are ignored. */
export function agentReducer(state: AgentState, event: AgentEvent): AgentState {
  switch (event.type) {
    case "focus":
      return state.mood === "idle" ? { ...state, mood: "listening" } : state;
    case "blur":
      return state.mood === "listening" ? { ...state, mood: "idle" } : state;
    case "submit": {
      const prompt = event.prompt.trim();
      if (!prompt || state.mood === "thinking") return state;
      return { mood: "thinking", pending: prompt };
    }
    case "answer":
      return state.mood === "thinking" ? { mood: "speaking", pending: null } : state;
    case "fail":
      return state.mood === "thinking" ? { mood: "error", pending: null } : state;
    case "done":
      return state.mood === "speaking" || state.mood === "error" ? { mood: "idle", pending: null } : state;
    default:
      return state;
  }
}

/** Hologram tint per mood, as linear RGB 0..1. */
export const MOOD_COLORS: Record<AgentMood, readonly [number, number, number]> = {
  idle: [0.35, 0.88, 1],
  listening: [0.38, 1, 0.82],
  thinking: [0.72, 0.52, 1],
  speaking: [0.5, 0.95, 1],
  error: [1, 0.42, 0.48],
};

/** Read-only checks the agent offers as one-tap prompts. */
export const AGENT_QUICK_PROMPTS: readonly string[] = [
  "Check all media sources",
  "Check metadata providers",
  "Run network diagnostics",
  "Discover media folders",
];

// ---------------------------------------------------------------- head ----

export const HEAD_REGION = { skull: 0, eye: 1, mouth: 2, jaw: 3 } as const;

export interface HeadCloud {
  /** x, y, z per point; the head spans roughly -1..1 on each axis. */
  positions: Float32Array;
  /** HEAD_REGION value per point. */
  regions: Float32Array;
  count: number;
}

/** Lips and brows drawn as bright arcs on the face so expressions read. */
const FEATURE_ARCS: ReadonlyArray<{ cx: number; y: number; halfWidth: number; bend: number; region: number; points: number }> = [
  { cx: 0, y: -0.37, halfWidth: 0.19, bend: -0.025, region: 2, points: 46 }, // upper lip
  { cx: 0, y: -0.43, halfWidth: 0.17, bend: 0.03, region: 3, points: 40 }, // lower lip
  { cx: -0.27, y: 0.29, halfWidth: 0.12, bend: 0.03, region: 0, points: 26 }, // brows
  { cx: 0.27, y: 0.29, halfWidth: 0.12, bend: 0.03, region: 0, points: 26 },
];

/** Points added for lips and brows on top of the surface, eyes and neck. */
export const FEATURE_POINT_COUNT = FEATURE_ARCS.reduce((sum, arc) => sum + arc.points, 0);

/** Front of the head surface at (x, y), matching the ellipsoid in buildHeadCloud. */
function faceDepth(x: number, y: number): number {
  const inside = 1 - (x / 0.78) ** 2 - (y / 1.02) ** 2;
  return 0.86 * Math.sqrt(Math.max(0, inside));
}

const EYE_CENTERS: ReadonlyArray<readonly [number, number, number]> = [
  [-0.27, 0.14, 0.74],
  [0.27, 0.14, 0.74],
];

/**
 * Builds a stylised head as a point cloud: a Fibonacci-sampled ellipsoid
 * shaped into skull, cheeks, nose and jaw, a dense cluster per eye, lips, brows and a neck.
 * Deterministic for a given count.
 */
export function buildHeadCloud(surfacePoints = 3800, eyePoints = 90, neckPoints = 420): HeadCloud {
  const surface = Math.max(0, Math.floor(surfacePoints));
  const eyes = Math.max(0, Math.floor(eyePoints));
  const neck = Math.max(0, Math.floor(neckPoints));
  const count = surface + eyes * EYE_CENTERS.length + neck + FEATURE_POINT_COUNT;
  const positions = new Float32Array(count * 3);
  const regions = new Float32Array(count);
  const golden = Math.PI * (3 - Math.sqrt(5));

  for (let i = 0; i < surface; i += 1) {
    const uy = surface === 1 ? 0 : 1 - (i / (surface - 1)) * 2;
    const radius = Math.sqrt(Math.max(0, 1 - uy * uy));
    const theta = golden * i;
    // Cranium a little wider than the face.
    let x = Math.cos(theta) * radius * 0.78 * (uy > 0.25 ? 1 + 0.07 * (uy - 0.25) : 1);
    let y = uy * 1.02;
    let z = Math.sin(theta) * radius * 0.86;
    let region: number = HEAD_REGION.skull;

    // Jaw narrows toward the chin, and the chin comes forward a little.
    if (y < -0.2) x *= 1 - 0.36 * ((-0.2 - y) / 0.82);
    if (y < -0.62 && z > 0) z += 0.07;
    const front = z > 0.35;
    // Nose ridge.
    if (front && Math.abs(x) < 0.13 && y > -0.3 && y < 0.16) {
      z += 0.13 * (1 - Math.abs(x) / 0.13) * (1 - Math.abs(y + 0.07) / 0.23);
    }
    // Eye sockets sit slightly inside the face.
    for (const [ex, ey] of EYE_CENTERS) {
      if (front && Math.hypot(x - ex, y - ey) < 0.13) z -= 0.05;
    }
    if (front && Math.abs(x) < 0.3 && y < -0.33 && y > -0.47) region = HEAD_REGION.mouth;
    else if (front && y <= -0.47) region = HEAD_REGION.jaw;

    positions.set([x, y, z], i * 3);
    regions[i] = region;
  }

  let offset = surface;
  for (const [cx, cy, cz] of EYE_CENTERS) {
    for (let j = 0; j < eyes; j += 1) {
      const angle = golden * j;
      const r = 0.075 * Math.sqrt((j + 0.5) / eyes);
      positions.set([cx + Math.cos(angle) * r, cy + Math.sin(angle) * r * 0.6, cz + 0.02], offset * 3);
      regions[offset] = HEAD_REGION.eye;
      offset += 1;
    }
  }
  // Neck: a slightly flared cylinder under the jaw, fading into the stage.
  for (let k = 0; k < neck; k += 1) {
    const t = (k + 0.5) / neck;
    const angle = golden * k * 7;
    const y = -0.78 - t * 0.4;
    const r = 0.3 + t * 0.1;
    positions.set([Math.cos(angle) * r, y, Math.sin(angle) * r * 0.9 - 0.05], offset * 3);
    regions[offset] = HEAD_REGION.skull;
    offset += 1;
  }
  for (const arc of FEATURE_ARCS) {
    for (let k = 0; k < arc.points; k += 1) {
      const t = arc.points === 1 ? 0 : (k / (arc.points - 1)) * 2 - 1;
      const x = arc.cx + t * arc.halfWidth;
      const y = arc.y + arc.bend * (1 - t * t);
      positions.set([x, y, faceDepth(x * 1.15, y) + 0.03], offset * 3);
      regions[offset] = arc.region;
      offset += 1;
    }
  }
  return { positions, regions, count };
}

export interface HeadPose {
  /** Turn around the vertical axis, radians. */
  yaw: number;
  /** 0 closed .. 1 fully open. */
  mouth: number;
  /** Distance of the camera from the head centre. */
  camera?: number;
}

export const HEAD_CAMERA_DISTANCE = 3.6;
export const JAW_DROP = 0.13;

/**
 * Moves one head point for the pose and projects it to normalised device
 * coordinates (-1..1, y up). `scale` is the perspective factor used for the
 * point size. The WebGL vertex shader in HoloHead.tsx mirrors this.
 */
export function projectHeadPoint(
  x: number,
  y: number,
  z: number,
  region: number,
  pose: HeadPose,
  aspect = 1,
): { x: number; y: number; scale: number } {
  const mouth = Math.min(1, Math.max(0, pose.mouth));
  let py = y;
  if (region === HEAD_REGION.jaw) py -= mouth * JAW_DROP;
  else if (region === HEAD_REGION.mouth) py -= mouth * JAW_DROP * 0.5;
  const cos = Math.cos(pose.yaw);
  const sin = Math.sin(pose.yaw);
  const rx = x * cos + z * sin;
  const rz = -x * sin + z * cos;
  const depth = (pose.camera ?? HEAD_CAMERA_DISTANCE) - rz;
  const focal = 2.4;
  const safeAspect = aspect > 0 ? aspect : 1;
  return { x: (rx * focal) / depth / safeAspect, y: (py * focal) / depth, scale: focal / depth };
}

// --------------------------------------------------------------- speech ----

/**
 * Mouth openness over time for an answer, one value per `stepMs`. Vowels open
 * the mouth, consonants half-open it, spaces and punctuation close it.
 */
export function mouthEnvelope(text: string, stepMs = 60, maxMs = 7000): number[] {
  const steps = Math.min(Math.ceil(maxMs / stepMs), text.length);
  const values: number[] = [];
  for (let i = 0; i < steps; i += 1) {
    const ch = text[i].toLowerCase();
    if (/[aeiouy]/.test(ch)) values.push(0.85);
    else if (/[a-z0-9]/.test(ch)) values.push(0.35);
    else values.push(0);
  }
  values.push(0);
  return values;
}

// -------------------------------------------------------------- answers ----

const MAX_ANSWER = 600;

function trimAnswer(text: string): string {
  const clean = text.replace(/\s+/g, " ").trim();
  return clean.length > MAX_ANSWER ? `${clean.slice(0, MAX_ANSWER - 1)}…` : clean;
}

function humanKey(key: string): string {
  return key.replace(/[_-]+/g, " ").trim();
}

/** Turns whatever ai_query returned into one or two sentences to say. */
export function summarizeAgentResult(result: unknown): string {
  if (result === null || result === undefined) return "Done.";
  if (typeof result === "string") return trimAnswer(result) || "Done.";
  if (Array.isArray(result)) return `Found ${result.length} result${result.length === 1 ? "" : "s"}.`;
  if (typeof result !== "object") return trimAnswer(String(result));

  const record = result as Record<string, unknown>;
  if (record.status === "error") {
    const detail = typeof record.message === "string" ? trimAnswer(record.message) : "";
    return detail ? `That didn't work: ${detail}` : "That didn't work.";
  }
  if (typeof record.message === "string" && record.message.trim()) return trimAnswer(record.message);
  if (typeof record.summary === "string" && record.summary.trim()) return trimAnswer(record.summary);

  // Checks such as network diagnostics: { dns: { test, success }, ... }
  const checks = Object.values(record).filter(
    (value): value is { test: string; success: boolean } =>
      typeof value === "object" &&
      value !== null &&
      typeof (value as { test?: unknown }).test === "string" &&
      typeof (value as { success?: unknown }).success === "boolean",
  );
  if (checks.length > 0) {
    const failed = checks.filter((check) => !check.success).map((check) => check.test);
    return failed.length === 0
      ? `All ${checks.length} checks passed.`
      : `${checks.length - failed.length} of ${checks.length} checks passed. Failed: ${failed.join(", ")}.`;
  }

  const counts = Object.entries(record)
    .filter(([, value]) => typeof value === "number" && Number.isFinite(value))
    .slice(0, 4)
    .map(([key, value]) => `${humanKey(key)} ${value}`);
  if (counts.length > 0) return `Finished: ${counts.join(", ")}.`;
  return "Done.";
}

// ---------------------------------------------------------- Claude brain ----

/** Progress the back end streams while Claude works (ai_agent.rs emit_activity). */
export type AgentActivityPhase = "thinking" | "searching" | "acting" | "idle";

const TOOL_ACTIVITY: Record<string, string> = {
  search_library: "Searching your library",
  library_overview: "Reading your library",
  get_media_item: "Reading the details",
  view_poster: "Looking at the poster",
  run_diagnostics: "Running diagnostics",
};

/** Status line under the agent's name while it works. */
export function activityLabel(phase: AgentActivityPhase, tool: string | null): string {
  if (phase === "acting") return "Preparing an action for you";
  if (phase === "searching") return (tool && TOOL_ACTIVITY[tool]) || "Checking";
  if (phase === "thinking") return "Thinking";
  return "Ready";
}

/** The Messages API accepts these image types, up to 5 MB each. */
export const AGENT_IMAGE_TYPES: readonly string[] = ["image/jpeg", "image/png", "image/gif", "image/webp"];
export const AGENT_IMAGE_MAX_BYTES = 5 * 1024 * 1024;

/** Why an attachment can't be sent, or null when it can. */
export function imageAttachmentError(type: string, size: number): string | null {
  if (!AGENT_IMAGE_TYPES.includes(type)) return "Attach a JPEG, PNG, GIF or WebP image.";
  if (size <= 0) return "That image is empty.";
  // Base64 grows the payload by a third; the limit applies to the encoded data.
  if (Math.ceil(size / 3) * 4 > AGENT_IMAGE_MAX_BYTES) return "Images must be under about 3.7 MB.";
  return null;
}

/** Strips the data-URL prefix FileReader adds, leaving bare base64. */
export function base64FromDataUrl(dataUrl: string): string {
  const comma = dataUrl.indexOf(",");
  return comma >= 0 ? dataUrl.slice(comma + 1) : dataUrl;
}
