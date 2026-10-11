// CinaVault Premium — photo portrait for the AI agent's head.
// The user picks a photo and marks both eyes and the mouth once. From those
// three points the photo is laid over a grid mesh whose vertices carry
// weights for the jaw, eyelids, brows and an ellipsoid depth, so a vertex
// shader can open the mouth, blink, raise brows and turn the head with
// parallax. Everything here is pure and runs offline; the photo itself is
// kept in app data by agent_avatar.rs and never bundled with the app.
import type { AgentMood } from "./holoAgent";

export interface PortraitPoint {
  /** 0..1 across the photo, left to right. */
  x: number;
  /** 0..1 down the photo, top to bottom. */
  y: number;
}

/** Eye and mouth centres on the photo, as the viewer sees them. */
export interface PortraitCalibration {
  leftEye: PortraitPoint;
  rightEye: PortraitPoint;
  mouth: PortraitPoint;
}

export interface PortraitPose {
  /** Head turn in radians; positive turns the face to the viewer's right. */
  yaw: number;
  /** Head nod in radians; positive tips the chin down. */
  pitch: number;
  /** Head tilt in radians. */
  roll: number;
  /** Eyelids 0 (open) .. 1 (closed). */
  blink: number;
  /** Brows -1 (frown) .. 1 (raised). */
  brow: number;
  /** Jaw 0 (closed) .. 1 (open). */
  mouth: number;
}

export interface PortraitMesh {
  /** Texture coordinates per vertex (x, y), also the rest position. */
  uvs: Float32Array;
  /** Per vertex: depth, jaw, eyelid, brow, mouth-gap mask, head mask. */
  weights: Float32Array;
  indices: Uint32Array;
  vertexCount: number;
}

/** Number of floats per vertex in PortraitMesh.weights. */
export const PORTRAIT_WEIGHTS = 6;
/** How far the jaw drops at a fully open mouth, in eye-to-mouth lengths. */
export const PORTRAIT_JAW_DROP = 0.14;
/** How far an eyelid travels at a full blink, in eye-to-eye lengths. */
export const PORTRAIT_LID_TRAVEL = 0.07;
/** Brow lift at brow = 1, in eye-to-eye lengths. */
export const PORTRAIT_BROW_LIFT = 0.045;
/** Parallax strength of a head turn on the most forward point. */
export const PORTRAIT_PARALLAX = 0.12;

export const PORTRAIT_IMAGE_TYPES: readonly string[] = ["image/jpeg", "image/png", "image/webp"];
export const PORTRAIT_MAX_BYTES = 12 * 1024 * 1024;

/** Where the markers start for a typical head-and-shoulders photo. */
export function defaultCalibration(): PortraitCalibration {
  return {
    leftEye: { x: 0.4, y: 0.4 },
    rightEye: { x: 0.6, y: 0.4 },
    mouth: { x: 0.5, y: 0.6 },
  };
}

/** Why a picked file can't be used as the portrait, or null when it can. */
export function portraitImageError(type: string, size: number): string | null {
  if (!PORTRAIT_IMAGE_TYPES.includes(type)) return "Choose a JPEG, PNG or WebP photo.";
  if (size <= 0) return "That photo is empty.";
  if (size > PORTRAIT_MAX_BYTES) return "Photos must be under 12 MB.";
  return null;
}

/**
 * Why a set of markers can't drive the face, or null when it can. `aspect`
 * is the photo's width/height; agent_avatar.rs applies the same rules.
 */
export function calibrationError(cal: PortraitCalibration, aspect = 1): string | null {
  const points = [cal.leftEye, cal.rightEye, cal.mouth];
  if (points.some((p) => !Number.isFinite(p.x) || !Number.isFinite(p.y) || p.x < 0 || p.x > 1 || p.y < 0 || p.y > 1)) {
    return "Keep the markers on the photo.";
  }
  if (cal.leftEye.x >= cal.rightEye.x) return "The left eye marker must be left of the right eye marker.";
  const frame = faceFrame(cal, aspect);
  if (frame.eyeDistance < 0.04) return "Move the eye markers apart, onto each eye.";
  // The mouth sits below the eyes along the face's own down direction.
  if (frame.mouthDistance < frame.eyeDistance * 0.45) return "Move the mouth marker down onto the mouth.";
  if (frame.mouthDistance > frame.eyeDistance * 2.5) return "The mouth marker is too far from the eyes.";
  return null;
}

interface FaceFrame {
  /** Midpoint between the eyes. */
  origin: PortraitPoint;
  /** Unit vector from the left eye to the right eye. */
  across: PortraitPoint;
  /** Unit vector pointing down the face (perpendicular to across). */
  down: PortraitPoint;
  eyeDistance: number;
  /** Distance from the eye midpoint to the mouth along `down`. */
  mouthDistance: number;
}

export function faceFrame(cal: PortraitCalibration, aspect = 1): FaceFrame {
  // Work in a space where one unit is the same length across and down the photo.
  const sx = aspect;
  const ex = (cal.rightEye.x - cal.leftEye.x) * sx;
  const ey = cal.rightEye.y - cal.leftEye.y;
  const eyeDistance = Math.hypot(ex, ey);
  const across = eyeDistance > 0 ? { x: ex / eyeDistance, y: ey / eyeDistance } : { x: 1, y: 0 };
  const down = { x: -across.y, y: across.x };
  const origin = { x: (cal.leftEye.x + cal.rightEye.x) / 2, y: (cal.leftEye.y + cal.rightEye.y) / 2 };
  const mouthDistance = (cal.mouth.x - origin.x) * sx * down.x + (cal.mouth.y - origin.y) * down.y;
  return { origin, across, down, eyeDistance, mouthDistance };
}

function smoothstep(edge0: number, edge1: number, value: number): number {
  const t = Math.min(1, Math.max(0, (value - edge0) / (edge1 - edge0)));
  return t * t * (3 - 2 * t);
}

const gauss = (value: number, width: number) => Math.exp(-(value * value) / (2 * width * width));

/**
 * Weights for one point of the photo. `u` and `v` are photo coordinates;
 * the result is [depth, jaw, eyelid, brow, mouthGap, head], each 0..1.
 */
export function portraitWeights(
  cal: PortraitCalibration,
  u: number,
  v: number,
  aspect = 1,
): [number, number, number, number, number, number] {
  const f = faceFrame(cal, aspect);
  const d = f.eyeDistance;
  const m = f.mouthDistance;
  const dx = (u - f.origin.x) * aspect;
  const dy = v - f.origin.y;
  // Face coordinates: a across (right +), b down (chin +), from the eye midpoint.
  const a = dx * f.across.x + dy * f.across.y;
  const b = dx * f.down.x + dy * f.down.y;

  // Ellipsoid face, centred a little below the eyes.
  const cx = 0;
  const cy = m * 0.4;
  const rx = d * 1.05;
  const ry = m * 1.55;
  const ex = (a - cx) / rx;
  const ey = (b - cy) / ry;
  const inside = 1 - (ex * ex + ey * ey);
  // Square-root bulge, eased to zero at the outline so no seam shows.
  let depth = inside > 0 ? Math.sqrt(inside) * smoothstep(0, 0.25, inside) : 0;
  // A soft nose ridge pushes the middle of the face forward.
  depth += 0.35 * gauss(a, d * 0.16) * gauss(b - m * 0.55, m * 0.3) * smoothstep(0, 0.3, inside);
  depth = Math.min(1, depth);

  // Jaw: everything from the lip line down to the chin, fading at the cheeks.
  const t = (b - m) / m; // 0 at the mouth, ~0.6 at the chin
  const sideJaw = gauss(a, d * 0.62);
  const jaw = smoothstep(-0.04, 0.06, t) * (1 - smoothstep(0.55, 0.85, t)) * sideJaw;

  // Upper eyelids: a band from just below each eye centre up to the crease.
  let lid = 0;
  let brow = 0;
  for (const side of [-1, 1]) {
    const ea = a - side * d * 0.5;
    const above = -b; // positive above the eye line
    lid = Math.max(lid, gauss(ea, d * 0.15) * smoothstep(-0.06 * d, 0.0, above) * (1 - smoothstep(0.05 * d, 0.13 * d, above)));
    brow = Math.max(brow, gauss(ea, d * 0.26) * gauss(above - 0.24 * d, 0.1 * d));
  }

  // The gap between the lips, darkened as the jaw opens.
  const gap = gauss(a, d * 0.2) * gauss(t, 0.03);

  // The whole head with hair, ears and neck: kept in natural colour, while
  // the rest of the photo fades into the hologram.
  const ha = a / (d * 1.4);
  const hb = (b - m * 0.2) / (m * 2.05);
  const neck = gauss(a, d * 0.55) * smoothstep(m * 1.0, m * 1.6, b) * (1 - smoothstep(m * 2.2, m * 2.8, b));
  const head = Math.max(1 - smoothstep(0.75, 1.05, Math.hypot(ha, hb)), neck);
  return [depth, jaw, lid, brow, gap, head];
}

/** A cols x rows grid over the whole photo with deformation weights. */
export function buildPortraitMesh(cal: PortraitCalibration, cols = 96, rows = 96, aspect = 1): PortraitMesh {
  const vertexCount = (cols + 1) * (rows + 1);
  const uvs = new Float32Array(vertexCount * 2);
  const weights = new Float32Array(vertexCount * PORTRAIT_WEIGHTS);
  for (let row = 0; row <= rows; row += 1) {
    for (let col = 0; col <= cols; col += 1) {
      const index = row * (cols + 1) + col;
      const u = col / cols;
      const v = row / rows;
      uvs[index * 2] = u;
      uvs[index * 2 + 1] = v;
      weights.set(portraitWeights(cal, u, v, aspect), index * PORTRAIT_WEIGHTS);
    }
  }
  const indices = new Uint32Array(cols * rows * 6);
  let k = 0;
  for (let row = 0; row < rows; row += 1) {
    for (let col = 0; col < cols; col += 1) {
      const i = row * (cols + 1) + col;
      indices.set([i, i + 1, i + cols + 1, i + 1, i + cols + 2, i + cols + 1], k);
      k += 6;
    }
  }
  return { uvs, weights, indices, vertexCount };
}

/** Where one photo point lands on screen (photo coordinates) for a pose. */
export function deformPortraitPoint(
  cal: PortraitCalibration,
  u: number,
  v: number,
  pose: PortraitPose,
  aspect = 1,
): PortraitPoint {
  const f = faceFrame(cal, aspect);
  const [depth, jaw, lid, brow] = portraitWeights(cal, u, v, aspect);
  // Offsets in face space (across, down), in square units.
  const down = jaw * pose.mouth * PORTRAIT_JAW_DROP * f.mouthDistance
    + lid * pose.blink * PORTRAIT_LID_TRAVEL * f.eyeDistance
    - brow * pose.brow * PORTRAIT_BROW_LIFT * f.eyeDistance;
  let x = (u - f.origin.x) * aspect + f.down.x * down;
  let y = v - f.origin.y + f.down.y * down;
  // Parallax: forward points slide further than the background when turning.
  x += Math.sin(pose.yaw) * depth * PORTRAIT_PARALLAX;
  y += Math.sin(pose.pitch) * depth * PORTRAIT_PARALLAX;
  // Tilt the whole picture around the eye midpoint.
  const c = Math.cos(pose.roll);
  const s = Math.sin(pose.roll);
  return { x: (x * c - y * s) / aspect + f.origin.x, y: x * s + y * c + f.origin.y };
}

/** Deterministic blinking: a quick blink every few seconds, sometimes twice. */
export function blinkAt(seconds: number): number {
  const period = 3.6;
  const cycle = Math.floor(seconds / period);
  // A cheap hash gives each cycle its own offset and double-blink chance.
  const hash = Math.abs(Math.sin(cycle * 12.9898) * 43758.5453) % 1;
  const local = seconds - cycle * period - hash * 1.4;
  const one = (at: number) => {
    const p = (local - at) / 0.16;
    return p < 0 || p > 1 ? 0 : 1 - Math.abs(p * 2 - 1);
  };
  return Math.max(one(0), hash > 0.7 ? one(0.3) : 0);
}

const NEUTRAL: PortraitPose = { yaw: 0, pitch: 0, roll: 0, blink: 0, brow: 0, mouth: 0 };

/** The head's pose for a mood at a moment. `mouth` comes from the speech envelope. */
export function portraitPose(mood: AgentMood, seconds: number, mouth = 0, reduceMotion = false): PortraitPose {
  if (reduceMotion) {
    return { ...NEUTRAL, brow: mood === "listening" ? 0.5 : mood === "error" ? -0.6 : 0, mouth: 0 };
  }
  const sway = { yaw: Math.sin(seconds * 0.5) * 0.05, pitch: Math.sin(seconds * 0.37) * 0.025 };
  const blink = blinkAt(seconds);
  switch (mood) {
    case "listening":
      return { ...sway, roll: 0.05, blink, brow: 0.55, mouth: 0 };
    case "thinking":
      // Eyes narrowed, gaze drifting up and side to side.
      return {
        yaw: Math.sin(seconds * 1.1) * 0.08,
        pitch: -0.06 + Math.sin(seconds * 0.8) * 0.015,
        roll: Math.sin(seconds * 0.6) * 0.02,
        blink: Math.max(0.3, blink),
        brow: 0.3,
        mouth: 0,
      };
    case "speaking":
      return {
        yaw: sway.yaw,
        pitch: sway.pitch + Math.sin(seconds * 5) * 0.025 * mouth,
        roll: 0,
        blink,
        brow: 0.15 + mouth * 0.25,
        mouth: Math.min(1, Math.max(0, mouth)),
      };
    case "error":
      return { yaw: Math.sin(seconds * 3) * 0.04, pitch: 0.03, roll: 0, blink, brow: -0.7, mouth: 0 };
    default:
      return { ...sway, roll: 0, blink, brow: 0, mouth: 0 };
  }
}

export interface PortraitView {
  /** Point of the photo (in photo coordinates) at the centre of the stage. */
  center: PortraitPoint;
  /** Visible height of the stage, in photo heights. */
  height: number;
}

/**
 * Frames the face on the stage so a full-body or off-centre photo still
 * shows a head-and-shoulders close-up. `aspect` is the photo's width/height.
 */
export function portraitView(cal: PortraitCalibration, aspect = 1): PortraitView {
  const f = faceFrame(cal, aspect);
  const m = Math.max(f.mouthDistance, 1e-3);
  // The face ellipse is centred 0.4 mouth-lengths below the eyes; frame a
  // touch lower so the neck and the hologram base fit under the chin.
  const offset = m * 0.75;
  return {
    center: { x: f.origin.x + (f.down.x * offset) / aspect, y: f.origin.y + f.down.y * offset },
    height: Math.min(1.5, m * 5.4),
  };
}
