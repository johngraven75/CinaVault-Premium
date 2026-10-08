// Audio Crossfade: routes a <video> element through a Web Audio gain node so
// playback can fade in on start and fade out on stop or next. Only elements
// loaded with crossOrigin="anonymous" are routed; anything else would play
// silence through Web Audio.
import { useCallback, useEffect, useRef } from "react";

const sleep = (ms: number) => new Promise<void>((resolve) => window.setTimeout(resolve, ms));

export function useAudioFade() {
  const contextRef = useRef<AudioContext | null>(null);
  const gainsRef = useRef(new WeakMap<HTMLMediaElement, GainNode>());

  const gainFor = useCallback((element: HTMLMediaElement): GainNode | null => {
    if (element.crossOrigin !== "anonymous") return null;
    const existing = gainsRef.current.get(element);
    if (existing) return existing;
    try {
      const context = contextRef.current ?? new AudioContext();
      contextRef.current = context;
      const gain = context.createGain();
      context.createMediaElementSource(element).connect(gain).connect(context.destination);
      gainsRef.current.set(element, gain);
      return gain;
    } catch (error) {
      console.warn("Audio fade unavailable:", error);
      return null;
    }
  }, []);

  /** Ramps from silence to full volume over `seconds` (0 = straight to full). */
  const fadeIn = useCallback(
    (element: HTMLMediaElement, seconds: number) => {
      const gain = seconds > 0 ? gainFor(element) : gainsRef.current.get(element);
      const context = contextRef.current;
      if (!gain || !context) return;
      void context.resume();
      const now = context.currentTime;
      gain.gain.cancelScheduledValues(now);
      if (seconds > 0) {
        gain.gain.setValueAtTime(0, now);
        gain.gain.linearRampToValueAtTime(1, now + seconds);
      } else {
        gain.gain.setValueAtTime(1, now);
      }
    },
    [gainFor],
  );

  /** Ramps to silence over `seconds` and resolves when done. */
  const fadeOut = useCallback(
    async (element: HTMLMediaElement, seconds: number) => {
      if (seconds <= 0 || element.paused) return;
      const gain = gainFor(element);
      const context = contextRef.current;
      if (!gain || !context) return;
      const now = context.currentTime;
      gain.gain.cancelScheduledValues(now);
      gain.gain.setValueAtTime(gain.gain.value, now);
      gain.gain.linearRampToValueAtTime(0, now + seconds);
      await sleep(seconds * 1000);
    },
    [gainFor],
  );

  useEffect(
    () => () => {
      void contextRef.current?.close();
    },
    [],
  );

  return { fadeIn, fadeOut };
}
