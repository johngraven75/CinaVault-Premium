// CinaVault Premium — photo-realistic head for the AI agent.
// The user's own photo is drawn on a deforming grid mesh (portraitAvatar.ts):
// the jaw follows the speech envelope, the eyelids blink, the brows move with
// the mood and the head turns with parallax. A light hologram pass (mood tint,
// scanlines, faded edges) keeps it in the panel's style. Without WebGL2 a 2D
// canvas draws the framed photo still. Under reduced motion one frame is drawn.
import { useEffect, useRef } from "react";
import type { MutableRefObject } from "react";
import { useReducedMotion } from "framer-motion";

import { MOOD_COLORS, type AgentMood } from "../../services/holoAgent";
import {
  PORTRAIT_BROW_LIFT,
  PORTRAIT_JAW_DROP,
  PORTRAIT_LID_TRAVEL,
  PORTRAIT_PARALLAX,
  PORTRAIT_WEIGHTS,
  buildPortraitMesh,
  faceFrame,
  portraitPose,
  portraitView,
  type PortraitCalibration,
} from "../../services/portraitAvatar";

interface PortraitHeadProps {
  mood: AgentMood;
  /** Photo as a data URL, loaded from app data. */
  src: string;
  calibration: PortraitCalibration;
  /** Mouth openness 0..1, written by the parent while the agent speaks. */
  mouthRef: MutableRefObject<number>;
  className?: string;
}

const VERTEX = `#version 300 es
in vec2 aUV;
in vec4 aWeights; // depth, jaw, lid, brow
in vec2 aMasks; // mouth gap, head
uniform float uImageAspect;
uniform float uStageAspect;
uniform vec2 uOrigin;
uniform vec2 uDown;
uniform float uEyeDist;
uniform float uMouthDist;
uniform vec2 uCenter;
uniform float uHeight;
uniform float uYaw;
uniform float uPitch;
uniform float uRoll;
uniform float uBlink;
uniform float uBrow;
uniform float uMouth;
out vec2 vUV;
out float vDepth;
out float vGap;
out float vHead;
out vec2 vClip;
void main() {
  float down = aWeights.y * uMouth * ${PORTRAIT_JAW_DROP.toFixed(3)} * uMouthDist
    + aWeights.z * uBlink * ${PORTRAIT_LID_TRAVEL.toFixed(3)} * uEyeDist
    - aWeights.w * uBrow * ${PORTRAIT_BROW_LIFT.toFixed(3)} * uEyeDist;
  vec2 p = vec2((aUV.x - uOrigin.x) * uImageAspect, aUV.y - uOrigin.y) + uDown * down;
  p += vec2(sin(uYaw), sin(uPitch)) * aWeights.x * ${PORTRAIT_PARALLAX.toFixed(3)};
  float c = cos(uRoll);
  float s = sin(uRoll);
  p = vec2(p.x * c - p.y * s, p.x * s + p.y * c);
  vec2 center = vec2((uCenter.x - uOrigin.x) * uImageAspect, uCenter.y - uOrigin.y);
  vec2 clip = (p - center) / (uHeight * 0.5);
  clip.x /= uStageAspect;
  clip.y = -clip.y;
  gl_Position = vec4(clip, 0.0, 1.0);
  vUV = aUV;
  vDepth = aWeights.x;
  vGap = aMasks.x;
  vHead = aMasks.y;
  vClip = clip;
}`;

const FRAGMENT = `#version 300 es
precision highp float;
in vec2 vUV;
in float vDepth;
in float vGap;
in float vHead;
in vec2 vClip;
uniform sampler2D uPhoto;
uniform vec3 uColor;
uniform float uTime;
uniform float uMouth;
uniform float uHolo;
out vec4 outColor;
void main() {
  vec3 photo = texture(uPhoto, vUV).rgb;
  // The parted lips show the dark mouth behind them.
  photo *= 1.0 - clamp(vGap * uMouth * 1.1, 0.0, 0.55);
  float lum = dot(photo, vec3(0.299, 0.587, 0.114));
  // The face keeps its natural colour; the photo's background turns to hologram.
  float holo = mix(0.75, uHolo, vHead);
  vec3 color = mix(photo, uColor * lum * 1.35, holo) + uColor * 0.035;
  float scan = 0.93 + 0.07 * sin(gl_FragCoord.y * 1.35 - uTime * 7.0);
  color *= scan;
  // Fade the photo's background so the head floats above the base ring.
  float edge = 1.0 - smoothstep(0.5, 0.95, length(vClip * vec2(1.15, 0.95)));
  float alpha = edge * mix(0.4, 0.96, vHead) * (0.95 + 0.03 * sin(uTime * 23.0));
  outColor = vec4(color * alpha, alpha);
}`;

function compile(gl: WebGL2RenderingContext, type: number, source: string): WebGLShader | null {
  const shader = gl.createShader(type);
  if (!shader) return null;
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    console.warn("PortraitHead shader failed to compile:", gl.getShaderInfoLog(shader));
    gl.deleteShader(shader);
    return null;
  }
  return shader;
}

function linkProgram(gl: WebGL2RenderingContext): WebGLProgram | null {
  const vs = compile(gl, gl.VERTEX_SHADER, VERTEX);
  const fs = compile(gl, gl.FRAGMENT_SHADER, FRAGMENT);
  if (!vs || !fs) return null;
  const program = gl.createProgram();
  if (!program) return null;
  gl.attachShader(program, vs);
  gl.attachShader(program, fs);
  gl.linkProgram(program);
  gl.deleteShader(vs);
  gl.deleteShader(fs);
  if (gl.getProgramParameter(program, gl.LINK_STATUS)) return program;
  console.warn("PortraitHead program failed to link:", gl.getProgramInfoLog(program));
  gl.deleteProgram(program);
  return null;
}

export default function PortraitHead({ mood, src, calibration, mouthRef, className }: PortraitHeadProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const fallbackRef = useRef<HTMLCanvasElement | null>(null);
  const moodRef = useRef<AgentMood>(mood);
  const reduceMotion = useReducedMotion() ?? false;
  moodRef.current = mood;

  useEffect(() => {
    const glCanvas = canvasRef.current;
    const fallbackCanvas = fallbackRef.current;
    if (!glCanvas || !fallbackCanvas) return;
    let canvas = glCanvas;
    let disposed = false;
    let frame = 0;
    let draw: (seconds: number) => void = () => {};
    let cleanup = () => {};
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const start = performance.now();
    const image = new Image();

    const resize = () => {
      canvas.width = Math.max(1, Math.round(canvas.clientWidth * dpr));
      canvas.height = Math.max(1, Math.round(canvas.clientHeight * dpr));
    };

    const setup = () => {
      if (disposed) return;
      const aspect = image.naturalWidth / Math.max(1, image.naturalHeight);
      const frameInfo = faceFrame(calibration, aspect);
      const view = portraitView(calibration, aspect);
      const gl = glCanvas.getContext("webgl2", { premultipliedAlpha: true, antialias: true, alpha: true });
      const program = gl && linkProgram(gl);

      if (gl && program) {
        const mesh = buildPortraitMesh(calibration, 96, 96, aspect);
        const masks = new Float32Array(mesh.vertexCount * 2);
        const fourWeights = new Float32Array(mesh.vertexCount * 4);
        for (let i = 0; i < mesh.vertexCount; i += 1) {
          fourWeights.set(mesh.weights.subarray(i * PORTRAIT_WEIGHTS, i * PORTRAIT_WEIGHTS + 4), i * 4);
          masks[i * 2] = mesh.weights[i * PORTRAIT_WEIGHTS + 4];
          masks[i * 2 + 1] = mesh.weights[i * PORTRAIT_WEIGHTS + 5];
        }
        const vao = gl.createVertexArray();
        gl.bindVertexArray(vao);
        const attribute = (name: string, data: Float32Array, size: number) => {
          const buffer = gl.createBuffer();
          gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
          gl.bufferData(gl.ARRAY_BUFFER, data, gl.STATIC_DRAW);
          const location = gl.getAttribLocation(program, name);
          gl.enableVertexAttribArray(location);
          gl.vertexAttribPointer(location, size, gl.FLOAT, false, 0, 0);
          return buffer;
        };
        const buffers = [attribute("aUV", mesh.uvs, 2), attribute("aWeights", fourWeights, 4), attribute("aMasks", masks, 2)];
        const indexBuffer = gl.createBuffer();
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, indexBuffer);
        gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, mesh.indices, gl.STATIC_DRAW);

        const texture = gl.createTexture();
        gl.bindTexture(gl.TEXTURE_2D, texture);
        gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, image);
        gl.generateMipmap(gl.TEXTURE_2D);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);

        const u = (name: string) => gl.getUniformLocation(program, name);
        gl.useProgram(program);
        gl.uniform1f(u("uImageAspect"), aspect);
        gl.uniform2f(u("uOrigin"), frameInfo.origin.x, frameInfo.origin.y);
        gl.uniform2f(u("uDown"), frameInfo.down.x, frameInfo.down.y);
        gl.uniform1f(u("uEyeDist"), frameInfo.eyeDistance);
        gl.uniform1f(u("uMouthDist"), frameInfo.mouthDistance);
        gl.uniform2f(u("uCenter"), view.center.x, view.center.y);
        gl.uniform1f(u("uHeight"), view.height);
        gl.uniform1i(u("uPhoto"), 0);
        gl.uniform1f(u("uHolo"), 0.18);
        const loc = {
          stage: u("uStageAspect"),
          yaw: u("uYaw"),
          pitch: u("uPitch"),
          roll: u("uRoll"),
          blink: u("uBlink"),
          brow: u("uBrow"),
          mouth: u("uMouth"),
          time: u("uTime"),
          color: u("uColor"),
        };
        gl.enable(gl.BLEND);
        gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
        draw = (seconds) => {
          const current = moodRef.current;
          const pose = portraitPose(current, seconds, mouthRef.current, reduceMotion);
          gl.viewport(0, 0, canvas.width, canvas.height);
          gl.clearColor(0, 0, 0, 0);
          gl.clear(gl.COLOR_BUFFER_BIT);
          gl.uniform1f(loc.stage, canvas.width / canvas.height);
          gl.uniform1f(loc.yaw, pose.yaw);
          gl.uniform1f(loc.pitch, pose.pitch);
          gl.uniform1f(loc.roll, pose.roll);
          gl.uniform1f(loc.blink, pose.blink);
          gl.uniform1f(loc.brow, pose.brow);
          gl.uniform1f(loc.mouth, pose.mouth);
          gl.uniform1f(loc.time, reduceMotion ? 0 : seconds);
          gl.uniform3fv(loc.color, MOOD_COLORS[current]);
          gl.drawElements(gl.TRIANGLES, mesh.indices.length, gl.UNSIGNED_INT, 0);
        };
        cleanup = () => {
          buffers.forEach((buffer) => gl.deleteBuffer(buffer));
          gl.deleteBuffer(indexBuffer);
          gl.deleteTexture(texture);
          gl.deleteVertexArray(vao);
          gl.deleteProgram(program);
        };
      } else {
        canvas = fallbackCanvas;
        glCanvas.hidden = true;
        fallbackCanvas.hidden = false;
        const ctx = canvas.getContext("2d");
        draw = (seconds) => {
          if (!ctx) return;
          const { width, height } = canvas;
          const bob = reduceMotion ? 0 : Math.sin(seconds * 0.8) * height * 0.006;
          const scale = height / (view.height * image.naturalHeight);
          const dx = width / 2 - view.center.x * image.naturalWidth * scale;
          const dy = height / 2 - view.center.y * image.naturalHeight * scale + bob;
          ctx.clearRect(0, 0, width, height);
          ctx.save();
          ctx.beginPath();
          ctx.ellipse(width / 2, height / 2, height * 0.42, height * 0.5, 0, 0, Math.PI * 2);
          ctx.clip();
          ctx.drawImage(image, dx, dy, image.naturalWidth * scale, image.naturalHeight * scale);
          ctx.restore();
        };
      }

      resize();
      if (reduceMotion) draw(0);
      else {
        const loop = (now: number) => {
          if (disposed) return;
          draw((now - start) / 1000);
          frame = requestAnimationFrame(loop);
        };
        frame = requestAnimationFrame(loop);
      }
    };

    const observer = new ResizeObserver(() => {
      resize();
      if (reduceMotion) draw(0);
    });
    observer.observe(glCanvas);
    observer.observe(fallbackCanvas);
    const redraw = () => draw(0);
    glCanvas.addEventListener("holo-redraw", redraw);
    image.onload = setup;
    image.onerror = () => console.warn("PortraitHead could not load the portrait photo.");
    image.src = src;

    return () => {
      disposed = true;
      image.onload = null;
      cancelAnimationFrame(frame);
      observer.disconnect();
      glCanvas.removeEventListener("holo-redraw", redraw);
      cleanup();
    };
  }, [src, calibration, reduceMotion, mouthRef]);

  // Under reduced motion the frame is redrawn only when the mood changes.
  useEffect(() => {
    if (!reduceMotion) return;
    canvasRef.current?.dispatchEvent(new Event("holo-redraw"));
  }, [mood, reduceMotion]);

  return (
    <>
      <canvas ref={canvasRef} className={className} data-mood={mood} data-portrait="photo" />
      <canvas ref={fallbackRef} className={className} data-mood={mood} hidden />
    </>
  );
}
