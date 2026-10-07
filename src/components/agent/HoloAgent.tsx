// CinaVault Premium — holographic AI agent: a floating orb that opens a
// projected 3D head and a chat. Prompts go to the ai_query command, which
// routes them to diagnostics, source checks, library automation or the
// configured AI model. Answers are typed out while the head speaks, and can
// be read aloud with the system voice.
import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import type { FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AnimatePresence, motion } from "framer-motion";
import { Send, Sparkles, Volume2, VolumeX, X } from "lucide-react";

import HoloHead from "./HoloHead";
import {
  AGENT_QUICK_PROMPTS,
  AGENT_START,
  agentReducer,
  mouthEnvelope,
  summarizeAgentResult,
} from "../../services/holoAgent";

interface ChatLine {
  id: number;
  role: "user" | "agent";
  text: string;
}

const VOICE_KEY = "cinavault.agent.voice";
const STEP_MS = 60;
const MOOD_LABEL = {
  idle: "Ready",
  listening: "Listening",
  thinking: "Thinking",
  speaking: "Answering",
  error: "Something went wrong",
} as const;

function readVoicePreference(): boolean {
  try {
    return window.localStorage.getItem(VOICE_KEY) === "on";
  } catch {
    return false;
  }
}

export default function HoloAgent() {
  const [open, setOpen] = useState(false);
  const [state, dispatch] = useReducer(agentReducer, AGENT_START);
  const [input, setInput] = useState("");
  const [lines, setLines] = useState<ChatLine[]>([
    { id: 0, role: "agent", text: "Hi, I'm your CinaVault agent. Ask me to check your sources, providers or network." },
  ]);
  const [voice, setVoice] = useState(readVoicePreference);
  const mouthRef = useRef(0);
  const nextId = useRef(1);
  const timer = useRef<number | null>(null);
  const logRef = useRef<HTMLDivElement | null>(null);

  const stopSpeaking = useCallback(() => {
    if (timer.current !== null) window.clearInterval(timer.current);
    timer.current = null;
    mouthRef.current = 0;
    if ("speechSynthesis" in window) window.speechSynthesis.cancel();
  }, []);

  useEffect(() => stopSpeaking, [stopSpeaking]);

  useEffect(() => {
    logRef.current?.scrollTo({ top: logRef.current.scrollHeight });
  }, [lines]);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open]);

  const toggleVoice = () => {
    setVoice((current) => {
      const next = !current;
      try {
        window.localStorage.setItem(VOICE_KEY, next ? "on" : "off");
      } catch {
        /* preference just isn't remembered */
      }
      if (!next && "speechSynthesis" in window) window.speechSynthesis.cancel();
      return next;
    });
  };

  /** Types the answer into the chat and moves the mouth along with it. */
  const speak = (text: string) => {
    stopSpeaking();
    const id = nextId.current++;
    setLines((prev) => [...prev, { id, role: "agent", text: "" }]);
    const envelope = mouthEnvelope(text, STEP_MS);
    const charsPerStep = Math.max(1, Math.ceil(text.length / Math.max(1, envelope.length - 1)));
    let step = 0;
    if (voice && "speechSynthesis" in window) {
      window.speechSynthesis.speak(new SpeechSynthesisUtterance(text));
    }
    timer.current = window.setInterval(() => {
      step += 1;
      mouthRef.current = envelope[Math.min(step, envelope.length - 1)] ?? 0;
      const shown = text.slice(0, step * charsPerStep);
      setLines((prev) => prev.map((line) => (line.id === id ? { ...line, text: shown } : line)));
      if (shown.length >= text.length) {
        if (timer.current !== null) window.clearInterval(timer.current);
        timer.current = null;
        mouthRef.current = 0;
        dispatch({ type: "done" });
      }
    }, STEP_MS);
  };

  const ask = async (prompt: string) => {
    const clean = prompt.trim();
    if (!clean || state.mood === "thinking") return;
    stopSpeaking();
    dispatch({ type: "submit", prompt: clean });
    setLines((prev) => [...prev, { id: nextId.current++, role: "user", text: clean }]);
    setInput("");
    try {
      const result = await invoke<unknown>("ai_query", { prompt: clean });
      dispatch({ type: "answer" });
      speak(summarizeAgentResult(result));
    } catch (error) {
      dispatch({ type: "fail" });
      speak(`That didn't work: ${String(error)}`);
    }
  };

  const onSubmit = (event: FormEvent) => {
    event.preventDefault();
    void ask(input);
  };

  return (
    <>
      <button
        type="button"
        className={`holo-agent-orb is-${state.mood}`}
        onClick={() => setOpen((value) => !value)}
        aria-label={open ? "Close the AI agent" : "Open the AI agent"}
        aria-expanded={open}
      >
        <Sparkles size={20} />
      </button>
      <AnimatePresence>
        {open && (
          <motion.section
            className="holo-agent-panel glass-panel"
            role="dialog"
            aria-label="CinaVault AI agent"
            initial={{ opacity: 0, y: 24, scale: 0.96 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 16, scale: 0.97 }}
            transition={{ duration: 0.22, ease: "easeOut" }}
          >
            <header className="holo-agent-header">
              <div>
                <div className="text-sm font-bold text-white">CinaVault Agent</div>
                <div className="text-xs text-cv-subtext" aria-live="polite">{MOOD_LABEL[state.mood]}</div>
              </div>
              <div className="flex items-center gap-1">
                <button
                  type="button"
                  className="cv-btn cv-btn-secondary px-2 py-1 text-xs"
                  onClick={toggleVoice}
                  aria-pressed={voice}
                  aria-label={voice ? "Turn voice off" : "Turn voice on"}
                >
                  {voice ? <Volume2 size={14} /> : <VolumeX size={14} />}
                </button>
                <button type="button" className="cv-btn cv-btn-secondary px-2 py-1 text-xs" onClick={() => setOpen(false)} aria-label="Close">
                  <X size={14} />
                </button>
              </div>
            </header>
            <div className={`holo-agent-stage is-${state.mood}`}>
              <span className="holo-agent-ring" aria-hidden="true" />
              <HoloHead mood={state.mood} mouthRef={mouthRef} className="holo-agent-canvas" />
            </div>
            <div ref={logRef} className="holo-agent-log" aria-live="polite">
              {lines.map((line) => (
                <p key={line.id} className={`holo-agent-line is-${line.role}`}>{line.text}</p>
              ))}
            </div>
            <div className="holo-agent-chips">
              {AGENT_QUICK_PROMPTS.map((prompt) => (
                <button
                  key={prompt}
                  type="button"
                  className="holo-agent-chip"
                  disabled={state.mood === "thinking"}
                  onClick={() => void ask(prompt)}
                >
                  {prompt}
                </button>
              ))}
            </div>
            <form className="holo-agent-form" onSubmit={onSubmit}>
              <input
                className="cv-input flex-1 text-sm"
                value={input}
                placeholder="Ask the agent…"
                aria-label="Message the AI agent"
                onChange={(event) => setInput(event.target.value)}
                onFocus={() => dispatch({ type: "focus" })}
                onBlur={() => dispatch({ type: "blur" })}
              />
              <button
                type="submit"
                className="cv-btn cv-btn-primary px-3"
                disabled={!input.trim() || state.mood === "thinking"}
                aria-label="Send"
              >
                <Send size={14} />
              </button>
            </form>
          </motion.section>
        )}
      </AnimatePresence>
    </>
  );
}
