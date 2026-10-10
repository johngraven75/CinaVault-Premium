// CinaVault Premium — holographic AI agent: a floating orb that opens a
// projected 3D head and a chat. With an Anthropic API key the agent is
// Claude (agent_chat in ai_agent.rs): it reads the library, looks at posters
// and attached images, runs diagnostics, and offers buttons for anything that
// changes the library. Without a key, prompts go to the ai_query command,
// which routes them to diagnostics, source checks, library automation or the
// configured local model. Answers are typed out while the head speaks, and
// can be read aloud with the system voice.
import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import type { ClipboardEvent, FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AnimatePresence, motion } from "framer-motion";
import { Check, Paperclip, RotateCcw, Send, Settings, Sparkles, Volume2, VolumeX, X } from "lucide-react";

import HoloHead from "./HoloHead";
import {
  AGENT_IMAGE_TYPES,
  AGENT_QUICK_PROMPTS,
  AGENT_START,
  activityLabel,
  agentReducer,
  mouthEnvelope,
  summarizeAgentResult,
} from "../../services/holoAgent";
import {
  ANTHROPIC_KEYS_URL,
  MODEL_LABELS,
  clearAgentApiKey,
  errorText,
  getAgentStatus,
  readAgentImage,
  resetAgent,
  runAgentAction,
  sendAgentMessage,
  setAgentApiKey,
  setAgentModel,
} from "../../services/agentBrain";
import type { AgentAction, AgentImageInput, AgentStatus } from "../../services/agentBrain";

type ActionState = "ready" | "running" | "done" | "failed";

interface ChatLine {
  id: number;
  role: "user" | "agent";
  text: string;
  /** Preview of an image the user attached (a data URL). */
  image?: string;
  actions?: AgentAction[];
}

interface Attachment {
  name: string;
  preview: string;
  input: AgentImageInput;
}

const VOICE_KEY = "cinavault.agent.voice";
const STEP_MS = 60;
const GREETING = "Hi, I'm your CinaVault agent. Ask me to check your sources, providers or network.";
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
  const [lines, setLines] = useState<ChatLine[]>([{ id: 0, role: "agent", text: GREETING }]);
  const [voice, setVoice] = useState(readVoicePreference);
  const [status, setStatus] = useState<AgentStatus | null>(null);
  const [activity, setActivity] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [keyDraft, setKeyDraft] = useState("");
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [attachment, setAttachment] = useState<Attachment | null>(null);
  const [attachError, setAttachError] = useState<string | null>(null);
  const [actionStates, setActionStates] = useState<Record<string, ActionState>>({});
  const mouthRef = useRef(0);
  const nextId = useRef(1);
  const timer = useRef<number | null>(null);
  /** The answer currently typing out, so an interruption can still show all of it. */
  /** Bumped by "New conversation", so a reply that was still on its way is dropped. */
  const session = useRef(0);
  const typing = useRef<{ id: number; text: string; actions?: AgentAction[] } | null>(null);
  const logRef = useRef<HTMLDivElement | null>(null);
  const fileRef = useRef<HTMLInputElement | null>(null);

  const claude = status?.configured === true;
  const busy = state.mood === "thinking";

  const stopSpeaking = useCallback(() => {
    if (timer.current !== null) window.clearInterval(timer.current);
    timer.current = null;
    const pending = typing.current;
    typing.current = null;
    if (pending) {
      setLines((prev) =>
        prev.map((line) => (line.id === pending.id ? { ...line, text: pending.text, actions: pending.actions } : line)),
      );
    }
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

  // Whether Claude is available is re-checked each time the panel opens, so a
  // key added through ANTHROPIC_API_KEY or the settings shows up right away.
  useEffect(() => {
    if (!open) return;
    let live = true;
    getAgentStatus()
      .then((next) => live && setStatus(next))
      .catch(() => live && setStatus(null));
    return () => {
      live = false;
    };
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
  const speak = (text: string, actions?: AgentAction[]) => {
    stopSpeaking();
    const id = nextId.current++;
    typing.current = { id, text, actions };
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
      const finished = shown.length >= text.length;
      setLines((prev) =>
        prev.map((line) => (line.id === id ? { ...line, text: shown, actions: finished ? actions : undefined } : line)),
      );
      if (finished) {
        if (timer.current !== null) window.clearInterval(timer.current);
        timer.current = null;
        typing.current = null;
        mouthRef.current = 0;
        dispatch({ type: "done" });
      }
    }, STEP_MS);
  };

  const ask = async (prompt: string) => {
    const clean = prompt.trim();
    if (busy) return;
    // The status check may still be in flight right after the panel opens.
    const useClaude = status ? status.configured : (await getAgentStatus().catch(() => null))?.configured === true;
    const image = useClaude ? attachment : null;
    if (!clean && !image) return;
    const asked = session.current;
    const shownPrompt = clean || "What is in this image?";
    stopSpeaking();
    dispatch({ type: "submit", prompt: shownPrompt });
    setLines((prev) => [...prev, { id: nextId.current++, role: "user", text: shownPrompt, image: image?.preview }]);
    setInput("");
    setAttachment(null);
    setAttachError(null);
    try {
      if (useClaude) {
        const reply = await sendAgentMessage(clean, image?.input ?? null, (phase, tool) =>
          setActivity(phase === "idle" ? null : activityLabel(phase, tool)),
        );
        setActivity(null);
        if (session.current !== asked) return;
        dispatch({ type: "answer" });
        speak(reply.text, reply.actions);
      } else {
        const result = await invoke<unknown>("ai_query", { prompt: clean });
        if (session.current !== asked) return;
        dispatch({ type: "answer" });
        speak(summarizeAgentResult(result));
      }
    } catch (error) {
      setActivity(null);
      if (session.current !== asked) return;
      dispatch({ type: "fail" });
      speak(`That didn't work: ${errorText(error)}`);
    }
  };

  const approve = async (action: AgentAction) => {
    if (actionStates[action.id] && actionStates[action.id] !== "ready") return;
    setActionStates((prev) => ({ ...prev, [action.id]: "running" }));
    try {
      const done = await runAgentAction(action.id);
      setActionStates((prev) => ({ ...prev, [action.id]: "done" }));
      setLines((prev) => [...prev, { id: nextId.current++, role: "agent", text: `${done}.` }]);
    } catch (error) {
      setActionStates((prev) => ({ ...prev, [action.id]: "failed" }));
      setLines((prev) => [...prev, { id: nextId.current++, role: "agent", text: `That didn't work: ${errorText(error)}` }]);
    }
  };

  const attach = async (file: File | undefined) => {
    if (!file) return;
    try {
      const imageInput = await readAgentImage(file);
      setAttachment({
        name: file.name || "Pasted image",
        preview: `data:${imageInput.mediaType};base64,${imageInput.data}`,
        input: imageInput,
      });
      setAttachError(null);
    } catch (error) {
      setAttachError(errorText(error));
    }
  };

  const onPaste = (event: ClipboardEvent<HTMLInputElement>) => {
    if (!claude) return;
    const file = Array.from(event.clipboardData.files).find((item) => AGENT_IMAGE_TYPES.includes(item.type));
    if (file) {
      event.preventDefault();
      void attach(file);
    }
  };

  const saveKey = async (event: FormEvent) => {
    event.preventDefault();
    try {
      setStatus(await setAgentApiKey(keyDraft));
      setKeyDraft("");
      setSettingsError(null);
      setSettingsOpen(false);
    } catch (error) {
      setSettingsError(errorText(error));
    }
  };

  const removeKey = async () => {
    try {
      setStatus(await clearAgentApiKey());
      setSettingsError(null);
    } catch (error) {
      setSettingsError(errorText(error));
    }
  };

  const chooseModel = async (model: string) => {
    try {
      setStatus(await setAgentModel(model));
    } catch (error) {
      setSettingsError(errorText(error));
    }
  };

  const newConversation = useCallback(async () => {
    session.current += 1;
    typing.current = null;
    stopSpeaking();
    setActivity(null);
    await resetAgent().catch(() => undefined);
    setActionStates({});
    setLines([{ id: nextId.current++, role: "agent", text: GREETING }]);
    dispatch({ type: "reset" });
  }, [stopSpeaking]);

  // A different profile gets its own conversation; nothing the last one saw carries over.
  useEffect(() => {
    const onProfileChange = () => void newConversation();
    window.addEventListener("cinavault:profile-changed", onProfileChange);
    return () => window.removeEventListener("cinavault:profile-changed", onProfileChange);
  }, [newConversation]);

  const onSubmit = (event: FormEvent) => {
    event.preventDefault();
    void ask(input);
  };

  const subtitle = activity ?? (state.mood === "idle" && status && !claude ? "Offline mode" : MOOD_LABEL[state.mood]);

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
                <div className="text-xs text-cv-subtext" aria-live="polite">{subtitle}</div>
              </div>
              <div className="flex items-center gap-1">
                <button
                  type="button"
                  className="cv-btn cv-btn-secondary px-2 py-1 text-xs"
                  onClick={() => setSettingsOpen((value) => !value)}
                  aria-pressed={settingsOpen}
                  aria-label="Agent settings"
                >
                  <Settings size={14} />
                </button>
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
            {settingsOpen ? (
              <div className="holo-agent-settings">
                {status?.keySource === "env" ? (
                  <p className="text-xs text-cv-subtext">Using the key from the ANTHROPIC_API_KEY environment variable.</p>
                ) : (
                  <form className="holo-agent-form" onSubmit={saveKey}>
                    <input
                      className="cv-input flex-1 text-sm"
                      type="password"
                      autoComplete="off"
                      value={keyDraft}
                      placeholder={claude ? "Replace Anthropic API key" : "Anthropic API key (sk-ant-…)"}
                      aria-label="Anthropic API key"
                      onChange={(event) => setKeyDraft(event.target.value)}
                    />
                    <button type="submit" className="cv-btn cv-btn-primary px-3 text-xs" disabled={!keyDraft.trim()}>
                      Save
                    </button>
                  </form>
                )}
                <p className="text-xs text-cv-subtext">
                  The key is kept in your system keychain and never shown again. Get one at{" "}
                  <a className="underline" href={ANTHROPIC_KEYS_URL} target="_blank" rel="noreferrer">
                    console.anthropic.com
                  </a>
                  .
                </p>
                {claude && (
                  <label className="flex flex-col gap-1 text-xs text-cv-subtext">
                    Model
                    <select
                      className="cv-input text-sm"
                      value={status.model}
                      onChange={(event) => void chooseModel(event.target.value)}
                    >
                      {status.models.map((model) => (
                        <option key={model} value={model}>
                          {MODEL_LABELS[model] ?? model}
                        </option>
                      ))}
                    </select>
                  </label>
                )}
                <div className="flex flex-wrap gap-2">
                  <button type="button" className="holo-agent-chip" onClick={() => void newConversation()}>
                    <RotateCcw size={11} className="mr-1 inline" />
                    New conversation
                  </button>
                  {status?.keySource === "keychain" && (
                    <button type="button" className="holo-agent-chip" onClick={() => void removeKey()}>
                      Remove key
                    </button>
                  )}
                </div>
                {settingsError && <p className="text-xs text-red-300" role="alert">{settingsError}</p>}
              </div>
            ) : (
              <div className={`holo-agent-stage is-${state.mood}`}>
                <span className="holo-agent-ring" aria-hidden="true" />
                <HoloHead mood={state.mood} mouthRef={mouthRef} className="holo-agent-canvas" />
              </div>
            )}
            <div ref={logRef} className="holo-agent-log" aria-live="polite">
              {lines.map((line) => (
                <div key={line.id} className={`holo-agent-line is-${line.role}`}>
                  {line.image && <img className="holo-agent-thumb" src={line.image} alt="Attached image" />}
                  <p>{line.text}</p>
                  {line.actions && line.actions.length > 0 && (
                    <div className="holo-agent-actions">
                      {line.actions.map((action) => {
                        const actionState = actionStates[action.id] ?? "ready";
                        return (
                          <button
                            key={action.id}
                            type="button"
                            className={`holo-agent-action is-${actionState}`}
                            disabled={actionState !== "ready"}
                            onClick={() => void approve(action)}
                          >
                            {actionState === "done" && <Check size={11} className="mr-1 inline" />}
                            {actionState === "running" ? "Working…" : action.label}
                          </button>
                        );
                      })}
                    </div>
                  )}
                </div>
              ))}
              {status && !claude && !settingsOpen && (
                <button type="button" className="holo-agent-upgrade" onClick={() => setSettingsOpen(true)}>
                  Add an Anthropic API key to unlock full chat, image recognition and library actions.
                </button>
              )}
            </div>
            <div className="holo-agent-chips">
              {AGENT_QUICK_PROMPTS.map((prompt) => (
                <button
                  key={prompt}
                  type="button"
                  className="holo-agent-chip"
                  disabled={busy}
                  onClick={() => void ask(prompt)}
                >
                  {prompt}
                </button>
              ))}
            </div>
            {attachment && (
              <div className="holo-agent-attachment">
                <img src={attachment.preview} alt="" />
                <span className="flex-1 truncate">{attachment.name}</span>
                <button type="button" onClick={() => setAttachment(null)} aria-label="Remove attached image">
                  <X size={12} />
                </button>
              </div>
            )}
            {attachError && <p className="text-xs text-red-300" role="alert">{attachError}</p>}
            <form className="holo-agent-form" onSubmit={onSubmit}>
              {claude && (
                <>
                  <input
                    ref={fileRef}
                    type="file"
                    accept={AGENT_IMAGE_TYPES.join(",")}
                    className="hidden"
                    onChange={(event) => {
                      void attach(event.target.files?.[0]);
                      event.target.value = "";
                    }}
                  />
                  <button
                    type="button"
                    className="cv-btn cv-btn-secondary px-2"
                    onClick={() => fileRef.current?.click()}
                    disabled={busy}
                    aria-label="Attach an image"
                  >
                    <Paperclip size={14} />
                  </button>
                </>
              )}
              <input
                className="cv-input flex-1 text-sm"
                value={input}
                placeholder="Ask the agent…"
                aria-label="Message the AI agent"
                onChange={(event) => setInput(event.target.value)}
                onPaste={onPaste}
                onFocus={() => dispatch({ type: "focus" })}
                onBlur={() => dispatch({ type: "blur" })}
              />
              <button
                type="submit"
                className="cv-btn cv-btn-primary px-3"
                disabled={(!input.trim() && !attachment) || busy}
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
