const STARTUP_AUDIO_PATH = "/audio/cinavault-startup.mp3";
const STARTUP_MESSAGE = "CinaVault Premier Server by Media Fire FL LLC";

let audioContext: AudioContext | null = null;
let initialized = false;
let lastInteractionAt = 0;
let startupRequested = false;

function getAudioContext(): AudioContext | null {
  if (typeof window === "undefined") return null;
  const contextWindow = window as typeof window & {
    webkitAudioContext?: typeof AudioContext;
  };
  const AudioContextCtor = window.AudioContext || contextWindow.webkitAudioContext;
  if (!AudioContextCtor) return null;
  audioContext ??= new AudioContextCtor();
  return audioContext;
}

function tone(
  frequency: number,
  durationMs: number,
  gainValue: number,
  type: OscillatorType = "sine",
  glideTo?: number,
): void {
  const context = getAudioContext();
  if (!context) return;
  if (context.state === "suspended") void context.resume();

  const oscillator = context.createOscillator();
  const gain = context.createGain();
  const now = context.currentTime;
  const duration = durationMs / 1000;

  oscillator.type = type;
  oscillator.frequency.setValueAtTime(frequency, now);
  if (glideTo) oscillator.frequency.exponentialRampToValueAtTime(glideTo, now + duration);

  gain.gain.setValueAtTime(0.0001, now);
  gain.gain.exponentialRampToValueAtTime(gainValue, now + 0.012);
  gain.gain.exponentialRampToValueAtTime(0.0001, now + duration);

  oscillator.connect(gain);
  gain.connect(context.destination);
  oscillator.start(now);
  oscillator.stop(now + duration + 0.02);
}

export function playSpaceClick(): void {
  tone(760, 80, 0.035, "triangle", 420);
}

export function playSpaceMenuSelect(): void {
  tone(240, 130, 0.04, "sine", 520);
  window.setTimeout(() => tone(720, 90, 0.025, "triangle", 980), 45);
}

export function playSpaceTransition(): void {
  tone(110, 240, 0.028, "sine", 330);
  window.setTimeout(() => tone(540, 190, 0.02, "sine", 180), 35);
}

function chooseDeepVoice(): SpeechSynthesisVoice | null {
  const voices = window.speechSynthesis?.getVoices?.() || [];
  const english = voices.filter((voice) => /^en(-|_)/i.test(voice.lang));
  const preferredNames = [
    "Guy",
    "Christopher",
    "Eric",
    "Davis",
    "Daniel",
    "George",
    "David",
    "Mark",
  ];
  return (
    preferredNames
      .map((name) => english.find((voice) => voice.name.includes(name)))
      .find(Boolean) ||
    english.find((voice) => /male|natural|neural/i.test(voice.name)) ||
    english[0] ||
    voices[0] ||
    null
  );
}

function speakStartupAnnouncement(): void {
  if (!("speechSynthesis" in window)) return;
  window.speechSynthesis.cancel();
  const utterance = new SpeechSynthesisUtterance(STARTUP_MESSAGE);
  utterance.voice = chooseDeepVoice();
  utterance.rate = 0.84;
  utterance.pitch = 0.72;
  utterance.volume = 0.92;
  window.speechSynthesis.speak(utterance);
}

export async function playStartupAnnouncement(): Promise<void> {
  if (startupRequested) return;
  startupRequested = true;
  const audio = new Audio(STARTUP_AUDIO_PATH);
  audio.preload = "auto";
  audio.volume = 0.92;
  try {
    await audio.play();
  } catch {
    speakStartupAnnouncement();
  }
}

function isMenuControl(target: HTMLElement): boolean {
  return Boolean(
    target.closest("nav") ||
      target.closest("[role='tablist']") ||
      target.closest("[data-sidebar-menu]") ||
      target.closest("[data-testid*='tab']"),
  );
}

export function initializeSpaceAudio(): () => void {
  if (initialized || typeof document === "undefined") return () => undefined;
  initialized = true;

  const handlePointer = (event: PointerEvent): void => {
    const now = Date.now();
    if (now - lastInteractionAt < 45) return;
    const target = event.target instanceof HTMLElement ? event.target : null;
    if (!target) return;
    const control = target.closest(
      "button, [role='button'], a, input[type='button'], input[type='submit']",
    );
    if (!(control instanceof HTMLElement) || control.hasAttribute("disabled")) return;
    lastInteractionAt = now;
    if (isMenuControl(control)) {
      playSpaceMenuSelect();
      window.setTimeout(playSpaceTransition, 70);
    } else {
      playSpaceClick();
    }
  };

  document.addEventListener("pointerdown", handlePointer, true);
  return () => {
    document.removeEventListener("pointerdown", handlePointer, true);
    initialized = false;
  };
}

export { STARTUP_AUDIO_PATH, STARTUP_MESSAGE };
