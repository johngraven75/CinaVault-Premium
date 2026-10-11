// CinaVault Premium — WebGL and animation-loop plumbing shared by the agent's
// heads (HoloHead's point cloud and PortraitHead's photo mesh).
import { useEffect } from "react";
import type { RefObject } from "react";

function compile(gl: WebGL2RenderingContext, type: number, source: string, label: string): WebGLShader | null {
  const shader = gl.createShader(type);
  if (!shader) return null;
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    console.warn(`${label} shader failed to compile:`, gl.getShaderInfoLog(shader));
    gl.deleteShader(shader);
    return null;
  }
  return shader;
}

/** Compiles and links a program, or returns null so the caller can fall back to 2D. */
export function linkHoloProgram(
  gl: WebGL2RenderingContext,
  vertex: string,
  fragment: string,
  label: string,
): WebGLProgram | null {
  const vs = compile(gl, gl.VERTEX_SHADER, vertex, label);
  const fs = compile(gl, gl.FRAGMENT_SHADER, fragment, label);
  if (!vs || !fs) return null;
  const program = gl.createProgram();
  if (!program) return null;
  gl.attachShader(program, vs);
  gl.attachShader(program, fs);
  gl.linkProgram(program);
  gl.deleteShader(vs);
  gl.deleteShader(fs);
  if (gl.getProgramParameter(program, gl.LINK_STATUS)) return program;
  console.warn(`${label} program failed to link:`, gl.getProgramInfoLog(program));
  gl.deleteProgram(program);
  return null;
}

/** Sizes a canvas's backing store to its CSS box at the given pixel ratio. */
export function fitCanvas(canvas: HTMLCanvasElement, dpr: number): void {
  canvas.width = Math.max(1, Math.round(canvas.clientWidth * dpr));
  canvas.height = Math.max(1, Math.round(canvas.clientHeight * dpr));
}

/**
 * Runs `draw` every frame, or once under reduced motion, and redraws a still
 * frame on resize or a "holo-redraw" event. Returns a function that stops it.
 */
export function startHoloLoop(
  eventTarget: HTMLCanvasElement,
  observed: HTMLCanvasElement[],
  reduceMotion: boolean,
  resize: () => void,
  draw: (seconds: number) => void,
): () => void {
  let frame = 0;
  let stopped = false;
  const start = performance.now();
  const loop = (now: number) => {
    if (stopped) return;
    draw((now - start) / 1000);
    frame = requestAnimationFrame(loop);
  };
  const observer = new ResizeObserver(() => {
    resize();
    if (reduceMotion) draw(0);
  });
  observed.forEach((canvas) => observer.observe(canvas));
  const redraw = () => draw(0);
  eventTarget.addEventListener("holo-redraw", redraw);
  resize();
  if (reduceMotion) draw(0);
  else frame = requestAnimationFrame(loop);
  return () => {
    stopped = true;
    cancelAnimationFrame(frame);
    observer.disconnect();
    eventTarget.removeEventListener("holo-redraw", redraw);
  };
}

/** Under reduced motion the still frame is redrawn only when the mood changes. */
export function useRedrawOnMood(canvasRef: RefObject<HTMLCanvasElement | null>, mood: string, reduceMotion: boolean): void {
  useEffect(() => {
    if (!reduceMotion) return;
    canvasRef.current?.dispatchEvent(new Event("holo-redraw"));
  }, [canvasRef, mood, reduceMotion]);
}
