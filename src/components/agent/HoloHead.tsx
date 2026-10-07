// CinaVault Premium — holographic point-cloud head for the AI agent.
// WebGL2 renders the cloud with additive glow and scanlines; a 2D canvas
// fallback draws the same projection when WebGL2 is unavailable. Under
// reduced motion a single still frame is drawn and no animation loop runs.
import { useEffect, useRef } from "react";
import type { MutableRefObject } from "react";
import { useReducedMotion } from "framer-motion";

import {
  HEAD_CAMERA_DISTANCE,
  JAW_DROP,
  MOOD_COLORS,
  buildHeadCloud,
  projectHeadPoint,
  type AgentMood,
} from "../../services/holoAgent";

interface HoloHeadProps {
  mood: AgentMood;
  /** Mouth openness 0..1, written by the parent while the agent speaks. */
  mouthRef: MutableRefObject<number>;
  className?: string;
}

const VERTEX = `#version 300 es
in vec3 aPosition;
in float aRegion;
uniform float uYaw;
uniform float uMouth;
uniform float uAspect;
uniform float uCamera;
uniform float uTime;
uniform float uPixel;
uniform float uEyeGlow;
out float vRegion;
out float vDepth;
void main() {
  vec3 p = aPosition;
  if (aRegion > 2.5) p.y -= uMouth * ${JAW_DROP.toFixed(3)};
  else if (aRegion > 1.5) p.y -= uMouth * ${(JAW_DROP * 0.5).toFixed(4)};
  float c = cos(uYaw);
  float s = sin(uYaw);
  vec3 r = vec3(p.x * c + p.z * s, p.y, -p.x * s + p.z * c);
  float depth = uCamera - r.z;
  float focal = 2.4;
  // Slow vertical drift reads as a projected hologram.
  float drift = sin(uTime * 0.9 + p.y * 3.0) * 0.004;
  gl_Position = vec4(r.x * focal / depth / uAspect + drift, r.y * focal / depth, 0.0, 1.0);
  float size = aRegion > 0.5 && aRegion < 1.5 ? 2.6 * uEyeGlow : 1.7;
  gl_PointSize = size * uPixel * (focal / depth) * 1.6;
  vRegion = aRegion;
  vDepth = clamp((r.z + 1.0) * 0.5, 0.0, 1.0);
}`;

const FRAGMENT = `#version 300 es
precision highp float;
in float vRegion;
in float vDepth;
uniform vec3 uColor;
uniform float uTime;
out vec4 outColor;
void main() {
  vec2 d = gl_PointCoord - 0.5;
  float r = length(d);
  if (r > 0.5) discard;
  float glow = smoothstep(0.5, 0.0, r);
  float scan = 0.72 + 0.28 * sin(gl_FragCoord.y * 1.35 - uTime * 7.0);
  float eye = vRegion > 0.5 && vRegion < 1.5 ? 1.6 : (vRegion > 1.5 && vRegion < 2.5 ? 1.45 : 1.0);
  float alpha = glow * scan * (0.25 + 0.75 * vDepth) * 0.55 * eye;
  outColor = vec4(uColor * alpha, alpha);
}`;

function compile(gl: WebGL2RenderingContext, type: number, source: string): WebGLShader | null {
  const shader = gl.createShader(type);
  if (!shader) return null;
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    console.warn("HoloHead shader failed to compile:", gl.getShaderInfoLog(shader));
    gl.deleteShader(shader);
    return null;
  }
  return shader;
}

function yawAt(time: number, mood: AgentMood): number {
  const speed = mood === "thinking" ? 1.6 : 0.45;
  return Math.sin(time * speed) * (mood === "thinking" ? 0.55 : 0.32);
}

export default function HoloHead({ mood, mouthRef, className }: HoloHeadProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  // A canvas keeps the first context it hands out, so the 2D fallback
  // draws on its own canvas when WebGL2 is missing or the shaders fail.
  const fallbackRef = useRef<HTMLCanvasElement | null>(null);
  const moodRef = useRef<AgentMood>(mood);
  const reduceMotion = useReducedMotion() ?? false;
  moodRef.current = mood;

  useEffect(() => {
    const glCanvas = canvasRef.current;
    const fallbackCanvas = fallbackRef.current;
    if (!glCanvas || !fallbackCanvas) return;
    let canvas = glCanvas;
    const cloud = buildHeadCloud();
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    let frame = 0;
    let disposed = false;
    const start = performance.now();

    const resize = () => {
      const { clientWidth, clientHeight } = canvas;
      canvas.width = Math.max(1, Math.round(clientWidth * dpr));
      canvas.height = Math.max(1, Math.round(clientHeight * dpr));
    };
    const gl = glCanvas.getContext("webgl2", { premultipliedAlpha: true, antialias: true, alpha: true });
    let draw: (seconds: number) => void;
    let cleanup = () => {};

    const program = gl && (() => {
      const vs = compile(gl, gl.VERTEX_SHADER, VERTEX);
      const fs = compile(gl, gl.FRAGMENT_SHADER, FRAGMENT);
      if (!vs || !fs) return null;
      const p = gl.createProgram();
      if (!p) return null;
      gl.attachShader(p, vs);
      gl.attachShader(p, fs);
      gl.linkProgram(p);
      gl.deleteShader(vs);
      gl.deleteShader(fs);
      if (gl.getProgramParameter(p, gl.LINK_STATUS)) return p;
      console.warn("HoloHead program failed to link:", gl.getProgramInfoLog(p));
      gl.deleteProgram(p);
      return null;
    })();

    if (gl && program) {
      const vao = gl.createVertexArray();
      gl.bindVertexArray(vao);
      const buffers = [cloud.positions, cloud.regions].map((data, index) => {
        const buffer = gl.createBuffer();
        gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
        gl.bufferData(gl.ARRAY_BUFFER, data, gl.STATIC_DRAW);
        const location = gl.getAttribLocation(program, index === 0 ? "aPosition" : "aRegion");
        gl.enableVertexAttribArray(location);
        gl.vertexAttribPointer(location, index === 0 ? 3 : 1, gl.FLOAT, false, 0, 0);
        return buffer;
      });
      const uniform = (name: string) => gl.getUniformLocation(program, name);
      const u = {
        yaw: uniform("uYaw"),
        mouth: uniform("uMouth"),
        aspect: uniform("uAspect"),
        camera: uniform("uCamera"),
        time: uniform("uTime"),
        pixel: uniform("uPixel"),
        eye: uniform("uEyeGlow"),
        color: uniform("uColor"),
      };
      gl.useProgram(program);
      gl.enable(gl.BLEND);
      gl.blendFunc(gl.ONE, gl.ONE);
      draw = (seconds) => {
        const current = moodRef.current;
        gl.viewport(0, 0, canvas.width, canvas.height);
        gl.clearColor(0, 0, 0, 0);
        gl.clear(gl.COLOR_BUFFER_BIT);
        gl.uniform1f(u.yaw, reduceMotion ? 0.25 : yawAt(seconds, current));
        gl.uniform1f(u.mouth, mouthRef.current);
        gl.uniform1f(u.aspect, canvas.width / canvas.height);
        gl.uniform1f(u.camera, HEAD_CAMERA_DISTANCE);
        gl.uniform1f(u.time, reduceMotion ? 0 : seconds);
        gl.uniform1f(u.pixel, dpr);
        gl.uniform1f(u.eye, current === "thinking" ? 1.5 + 0.4 * Math.sin(seconds * 6) : 1);
        gl.uniform3fv(u.color, MOOD_COLORS[current]);
        gl.drawArrays(gl.POINTS, 0, cloud.count);
      };
      cleanup = () => {
        buffers.forEach((buffer) => gl.deleteBuffer(buffer));
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
        const current = moodRef.current;
        const [r, g, b] = MOOD_COLORS[current];
        const { width, height } = canvas;
        ctx.clearRect(0, 0, width, height);
        ctx.fillStyle = `rgba(${Math.round(r * 255)}, ${Math.round(g * 255)}, ${Math.round(b * 255)}, 0.55)`;
        const pose = { yaw: reduceMotion ? 0.25 : yawAt(seconds, current), mouth: mouthRef.current };
        for (let i = 0; i < cloud.count; i += 1) {
          const p = projectHeadPoint(
            cloud.positions[i * 3],
            cloud.positions[i * 3 + 1],
            cloud.positions[i * 3 + 2],
            cloud.regions[i],
            pose,
            width / height,
          );
          const size = Math.max(1, p.scale * dpr * 1.4);
          ctx.fillRect(((p.x + 1) / 2) * width, ((1 - p.y) / 2) * height, size, size);
        }
      };
    }

    resize();
    const loop = (now: number) => {
      if (disposed) return;
      draw((now - start) / 1000);
      frame = requestAnimationFrame(loop);
    };
    const observer = new ResizeObserver(() => {
      resize();
      if (reduceMotion) draw(0);
    });
    observer.observe(canvas);
    const redraw = () => draw(0);
    glCanvas.addEventListener("holo-redraw", redraw);
    if (reduceMotion) draw(0);
    else frame = requestAnimationFrame(loop);

    return () => {
      disposed = true;
      cancelAnimationFrame(frame);
      observer.disconnect();
      glCanvas.removeEventListener("holo-redraw", redraw);
      cleanup();
    };
  }, [reduceMotion, mouthRef]);

  // Under reduced motion the frame is redrawn only when the mood changes.
  useEffect(() => {
    if (!reduceMotion) return;
    canvasRef.current?.dispatchEvent(new Event("holo-redraw"));
  }, [mood, reduceMotion]);

  return (
    <>
      <canvas ref={canvasRef} className={className} aria-hidden="true" data-mood={mood} />
      <canvas ref={fallbackRef} className={className} aria-hidden="true" data-mood={mood} hidden />
    </>
  );
}
