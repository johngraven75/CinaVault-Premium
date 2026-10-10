// Built-in player for library titles. Mounted once in App; playMedia() hands it
// every request through registerPlayer. It plays the original file through the
// asset protocol when the WebView can decode it, otherwise a loopback ffmpeg
// transcode, and hands the title to the external player when neither works
// (or Force Direct Play forbids transcoding). Decisions live in
// services/playerLogic.ts; this component wires them to two <video> elements
// (the second preloads the next title for gapless playback).
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { FC, SyntheticEvent } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Loader2, X } from "lucide-react";

import { useFeature } from "../../features/featureFlags";
import { registerPlayer, type PlayRequest } from "../../services/playback";
import {
  activeSkip,
  bufferHold,
  bufferPlan,
  bufferedAhead,
  chooseRoute,
  fadeSeconds,
  formatClock,
  nextUpRemaining,
  nextUpStart,
  resumeDecision,
  routeAfterError,
  shouldPreloadNext,
  shouldSaveProgress,
  skipWindows,
  transcodeSrc,
  type Route,
} from "../../services/playerLogic";
import { useAppStore, type MediaItem } from "../../store/appStore";
import PlayerControls from "./PlayerControls";
import {
  loadProgress,
  openExternally,
  preparePlayback,
  recordActivity,
  saveProgress,
  type PlayerSource,
} from "./playerApi";
import { useAudioFade } from "./useAudioFade";

interface Slot {
  token: number;
  /** Position in the queue. */
  index: number;
  item: MediaItem;
  source: PlayerSource;
  route: Route;
  /** Transcodes restart at `offset`; the element's clock starts at 0 there. */
  offset: number;
  src: string;
  /** Direct play seeks here once metadata loads. */
  startAt: number;
  resumedFrom: number | null;
}

type Prepared = { slot: Slot } | { external: string; item: MediaItem };
type SlotPair = [Slot | null, Slot | null];

const SKIP_INTRO_DEFAULTS = { introSeconds: 90, autoSkip: false };
const SKIP_CREDITS_DEFAULTS = { creditsSeconds: 120, autoSkip: false };
const NEXT_UP_DEFAULTS = { countdownSeconds: 10, secondsBeforeEnd: 30 };
const CROSSFADE_DEFAULTS = { seconds: 1.5 };
const BUFFER_DEFAULTS = { bufferAheadSeconds: 30, chunkKb: 256 };
const CONTROLS_HIDE_MS = 3000;
const TRANSCODE_SEEK_DEBOUNCE_MS = 300;

const num = (value: unknown, fallback: number) => {
  const n = Number(value);
  return Number.isFinite(n) ? n : fallback;
};

function shuffleAround<T>(list: T[], index: number): T[] {
  const rest = list.filter((_, i) => i !== index);
  for (let i = rest.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [rest[i], rest[j]] = [rest[j], rest[i]];
  }
  return [list[index], ...rest];
}

function timeRanges(ranges: TimeRanges): Array<[number, number]> {
  const out: Array<[number, number]> = [];
  for (let i = 0; i < ranges.length; i++) out.push([ranges.start(i), ranges.end(i)]);
  return out;
}

const LibraryPlayer: FC = () => {
  const addStatusMessage = useAppStore((state) => state.addStatusMessage);

  const cinema = useFeature("cinema_mode");
  const skipIntro = useFeature("skip_intro", SKIP_INTRO_DEFAULTS);
  const skipCredits = useFeature("skip_credits", SKIP_CREDITS_DEFAULTS);
  const nextUp = useFeature("next_up", NEXT_UP_DEFAULTS);
  const autoResume = useFeature("auto_resume");
  const crossfade = useFeature("crossfade", CROSSFADE_DEFAULTS);
  const gapless = useFeature("gapless");
  const streamBuffer = useFeature("stream_buffer", BUFFER_DEFAULTS);

  const plan = bufferPlan(streamBuffer.enabled, streamBuffer.config);
  const fade = fadeSeconds(crossfade.enabled, crossfade.config);
  const countdownSeconds = Math.max(1, num(nextUp.config.countdownSeconds, 10));

  // Latest switch values for async code.
  const live = useRef({ autoResume: autoResume.enabled, fade, nextUp: nextUp.enabled, gapless: gapless.enabled, plan });
  live.current = { autoResume: autoResume.enabled, fade, nextUp: nextUp.enabled, gapless: gapless.enabled, plan };

  const [queue, setQueue] = useState<MediaItem[] | null>(null);
  const [requestSource, setRequestSource] = useState<string | undefined>();
  const [slots, setSlots] = useState<SlotPair>([null, null]);
  const [active, setActive] = useState<0 | 1>(0);
  const [playing, setPlaying] = useState(false);
  const [time, setTime] = useState(0);
  const [elementDuration, setElementDuration] = useState(0);
  const [volume, setVolume] = useState(1);
  const [muted, setMuted] = useState(false);
  const [fullscreen, setFullscreen] = useState(false);
  const [buffering, setBuffering] = useState(false);
  const [resumeNotice, setResumeNotice] = useState<number | null>(null);
  const [controlsVisible, setControlsVisible] = useState(true);
  const [nextUpShownAt, setNextUpShownAt] = useState<number | null>(null);
  const [nextUpDismissed, setNextUpDismissed] = useState(false);

  const videoA = useRef<HTMLVideoElement>(null);
  const videoB = useRef<HTMLVideoElement>(null);
  const videos = useMemo(() => [videoA, videoB] as const, []);
  const containerRef = useRef<HTMLDivElement>(null);
  const restoreFocus = useRef<HTMLElement | null>(null);

  const queueRef = useRef(queue);
  queueRef.current = queue;
  const slotsRef = useRef(slots);
  slotsRef.current = slots;
  const activeRef = useRef(active);
  activeRef.current = active;
  const timeRef = useRef(time);
  timeRef.current = time;

  const tokenSeq = useRef(0);
  const wantPlay = useRef(false);
  const lastSave = useRef({ at: 0, position: 0 });
  const started = useRef(new Set<number>());
  const startApplied = useRef(new Set<number>());
  const autoSkipped = useRef(new Set<string>());
  const preloading = useRef<number | null>(null);
  const preloadFailed = useRef(new Set<number>());
  const busy = useRef(false);
  const hold = useRef<{ since: number } | null>(null);
  const pendingSeek = useRef<number | null>(null);
  const hideTimer = useRef<number | undefined>(undefined);
  const seekTimer = useRef<number | undefined>(undefined);

  const { fadeIn, fadeOut } = useAudioFade();

  const slot = slots[active];
  const probe = slot?.source.probe ?? null;
  const duration =
    probe && probe.duration > 0
      ? probe.duration
      : elementDuration > 0 && Number.isFinite(elementDuration)
        ? (slot?.offset ?? 0) + elementDuration
        : 0;
  const nextItem = queue && slot ? queue[slot.index + 1] : undefined;
  const previousItem = queue && slot ? queue[slot.index - 1] : undefined;

  // ── Preparing titles ──────────────────────────────────────────────────────

  const buildSlot = useCallback(
    (item: MediaItem, index: number, source: PlayerSource, route: Route, position: number, resumedFrom: number | null): Slot => {
      const base = { token: ++tokenSeq.current, index, item, source, route, resumedFrom };
      if (route.kind === "transcode" && source.transcodeUrl) {
        return { ...base, offset: position, src: transcodeSrc(source.transcodeUrl, position), startAt: 0 };
      }
      return { ...base, offset: 0, src: convertFileSrc(source.filePath), startAt: position };
    },
    [],
  );

  const prepare = useCallback(
    async (list: MediaItem[], index: number, startAt?: number): Promise<Prepared> => {
      const item = list[index];
      const source = await preparePlayback(item.id, item.file_path);
      const probeElement = document.createElement("video");
      const route = chooseRoute({
        filePath: source.filePath,
        probe: source.probe,
        canPlayType: (mime) => probeElement.canPlayType(mime),
        forceDirectPlay: source.forceDirectPlay,
        transcodeAvailable: Boolean(source.transcodeUrl),
      });
      if (route.kind === "external") return { external: route.reason, item };
      const resumeOn = live.current.autoResume;
      const progress = resumeOn && startAt === undefined ? await loadProgress(item.id ?? source.mediaId) : null;
      const decision = resumeDecision(progress, resumeOn, startAt);
      return {
        slot: buildSlot(item, index, source, route, decision.position, decision.reason === "resume" ? decision.position : null),
      };
    },
    [buildSlot],
  );

  // ── Progress ──────────────────────────────────────────────────────────────

  const slotDuration = useCallback((s: Slot, v: HTMLVideoElement | null) => {
    if (s.source.probe && s.source.probe.duration > 0) return s.source.probe.duration;
    const d = v?.duration ?? 0;
    return Number.isFinite(d) && d > 0 ? s.offset + d : 0;
  }, []);

  const persist = useCallback(
    (s: Slot, v: HTMLVideoElement | null, position?: number) => {
      const at = position ?? s.offset + (v?.currentTime ?? 0);
      const total = slotDuration(s, v);
      if (at <= 0 && total <= 0) return;
      lastSave.current = { at: Date.now(), position: at };
      void saveProgress(s.item.id ?? s.source.mediaId, at, total);
    },
    [slotDuration],
  );

  const resetPerTitle = useCallback(() => {
    setNextUpShownAt(null);
    setNextUpDismissed(false);
    setResumeNotice(null);
    setBuffering(false);
    setElementDuration(0);
    hold.current = null;
    pendingSeek.current = null;
    lastSave.current = { at: Date.now(), position: 0 };
  }, []);

  const activeVideo = useCallback(() => videos[activeRef.current].current, [videos]);

  /** Saves, fades out and pauses whatever is playing. */
  const stopCurrent = useCallback(async () => {
    const s = slotsRef.current[activeRef.current];
    const v = activeVideo();
    if (!s || !v) return;
    if (live.current.autoResume) persist(s, v);
    await fadeOut(v, live.current.fade);
    v.pause();
  }, [activeVideo, fadeOut, persist]);

  const close = useCallback(async () => {
    wantPlay.current = false;
    await stopCurrent();
    if (document.fullscreenElement) await document.exitFullscreen().catch(() => undefined);
    setQueue(null);
    setSlots([null, null]);
    setActive(0);
    setPlaying(false);
    resetPerTitle();
    preloadFailed.current.clear();
    restoreFocus.current?.focus?.();
  }, [resetPerTitle, stopCurrent]);

  const handOff = useCallback(
    async (item: MediaItem, reason: string) => {
      addStatusMessage(reason);
      await close();
      await openExternally(item.file_path).catch((error) => addStatusMessage(`External player failed: ${String(error)}`));
    },
    [addStatusMessage, close],
  );

  // ── Moving through the queue ──────────────────────────────────────────────

  const advance = useCallback(
    async (step: 1 | -1, ended = false) => {
      if (busy.current) return;
      busy.current = true;
      try {
        const list = queueRef.current;
        const here = activeRef.current;
        const current = slotsRef.current[here];
        if (!list || !current) return;
        const target = current.index + step;
        if (target < 0 || target >= list.length) {
          await close();
          return;
        }
        const v = videos[here].current;
        if (!ended) await stopCurrent();
        else v?.pause();
        resetPerTitle();
        wantPlay.current = true;

        const other = (1 - here) as 0 | 1;
        const preloaded = slotsRef.current[other];
        if (preloaded && preloaded.index === target) {
          setSlots((prev) => {
            const next: SlotPair = [...prev];
            next[here] = null;
            return next;
          });
          setActive(other);
          const nextVideo = videos[other].current;
          setTime(preloaded.offset + (nextVideo?.currentTime ?? 0));
          void nextVideo?.play().catch((error) => console.warn("Playback did not start:", error));
          return;
        }

        const prepared = await prepare(list, target).catch(
          (error): Prepared => ({ external: `Couldn't open ${list[target].title} in the app: ${String(error)}`, item: list[target] }),
        );
        if ("external" in prepared) {
          await handOff(prepared.item, prepared.external);
          return;
        }
        setTime(prepared.slot.offset + prepared.slot.startAt);
        setSlots((prev) => {
          const next: SlotPair = [...prev];
          next[here] = prepared.slot;
          next[other] = null;
          return next;
        });
      } finally {
        busy.current = false;
      }
    },
    [close, handOff, prepare, resetPerTitle, stopCurrent, videos],
  );

  // ── Requests from playMedia() ─────────────────────────────────────────────

  const handleRequest = useRef<(request: PlayRequest) => Promise<boolean>>(async () => false);
  handleRequest.current = async (request) => {
    const list = request.shuffle ? shuffleAround(request.queue, request.index) : request.queue;
    const index = request.shuffle ? 0 : request.index;
    let prepared: Prepared;
    try {
      prepared = await prepare(list, index, request.startAt);
    } catch (error) {
      console.warn("Built-in player declined; using the external player:", error);
      return false;
    }
    if ("external" in prepared) {
      addStatusMessage(prepared.external);
      return false;
    }
    if (queueRef.current) await stopCurrent();
    else restoreFocus.current = document.activeElement as HTMLElement | null;
    resetPerTitle();
    preloadFailed.current.clear();
    wantPlay.current = true;
    setRequestSource(request.source);
    setQueue(list);
    setTime(prepared.slot.offset + prepared.slot.startAt);
    setSlots([prepared.slot, null]);
    setActive(0);
    return true;
  };

  useEffect(() => registerPlayer((request) => handleRequest.current(request)), []);

  // ── Transport ─────────────────────────────────────────────────────────────

  const togglePlay = useCallback(() => {
    const v = activeVideo();
    if (!v) return;
    if (hold.current) {
      // Paused only to refill the buffer: the viewer is asking to pause.
      hold.current = null;
      wantPlay.current = false;
      setBuffering(false);
      setPlaying(false);
      return;
    }
    if (v.paused) {
      wantPlay.current = true;
      void v.play().catch((error) => console.warn("Playback did not start:", error));
    } else {
      wantPlay.current = false;
      v.pause();
    }
  }, [activeVideo]);

  const seek = useCallback(
    (seconds: number) => {
      const here = activeRef.current;
      const s = slotsRef.current[here];
      const v = videos[here].current;
      if (!s || !v) return;
      const total = slotDuration(s, v);
      const target = Math.max(0, total > 0 ? Math.min(seconds, total - 0.5) : seconds);
      setTime(target);
      setNextUpShownAt(null);
      if (s.route.kind === "direct") {
        v.currentTime = target;
        return;
      }
      // Transcodes restart ffmpeg at the new position; coalesce rapid seeks.
      pendingSeek.current = target;
      window.clearTimeout(seekTimer.current);
      seekTimer.current = window.setTimeout(() => {
        const at = pendingSeek.current;
        const latest = slotsRef.current[activeRef.current];
        const element = videos[activeRef.current].current;
        if (at === null || !latest?.source.transcodeUrl) return;
        const local = at - latest.offset;
        const nextSrc = transcodeSrc(latest.source.transcodeUrl, at);
        // Already transcoded (or the same stream): seek inside it instead of restarting ffmpeg.
        if (element && local >= 0 && (nextSrc === latest.src || bufferedAhead(timeRanges(element.buffered), local) > 0)) {
          pendingSeek.current = null;
          element.currentTime = local;
          return;
        }
        setSlots((prev) => {
          const next: SlotPair = [...prev];
          next[activeRef.current] = { ...latest, offset: at, src: nextSrc, startAt: 0 };
          return next;
        });
      }, TRANSCODE_SEEK_DEBOUNCE_MS);
    },
    [slotDuration, videos],
  );

  const jump = useCallback((delta: number) => seek(timeRef.current + delta), [seek]);

  const toggleFullscreen = useCallback(() => {
    if (document.fullscreenElement) void document.exitFullscreen().catch(() => undefined);
    else void containerRef.current?.requestFullscreen().catch((error) => console.warn("Full screen unavailable:", error));
  }, []);

  // ── Skip intro / credits and Next Up ──────────────────────────────────────

  const windows = useMemo(
    () =>
      skipWindows(probe?.chapters ?? [], duration, {
        introEnabled: skipIntro.enabled,
        creditsEnabled: skipCredits.enabled,
        introSeconds: num(skipIntro.config.introSeconds, 90),
        creditsSeconds: num(skipCredits.config.creditsSeconds, 120),
      }),
    [probe, duration, skipIntro.enabled, skipCredits.enabled, skipIntro.config.introSeconds, skipCredits.config.creditsSeconds],
  );
  const creditsChapter = useMemo(
    () =>
      skipWindows(probe?.chapters ?? [], duration, { introEnabled: false, creditsEnabled: true, introSeconds: 0, creditsSeconds: 0 })
        .credits,
    [probe, duration],
  );
  const nextUpAt = nextUpStart(duration, creditsChapter ?? windows.credits, {
    countdownSeconds,
    secondsBeforeEnd: num(nextUp.config.secondsBeforeEnd, 30),
  });
  const skipKind = slot ? activeSkip(time, windows) : null;

  const finish = useCallback(async () => {
    const s = slotsRef.current[activeRef.current];
    const v = activeVideo();
    if (s) persist(s, v, slotDuration(s, v));
    await close();
  }, [activeVideo, close, persist, slotDuration]);

  const doSkip = useCallback(
    (kind: "intro" | "credits") => {
      if (kind === "intro" && windows.intro) seek(windows.intro.end);
      if (kind === "credits") {
        if (nextItem) void advance(1);
        else void finish();
      }
    },
    [advance, finish, nextItem, seek, windows.intro],
  );

  useEffect(() => {
    if (!skipKind || !slot) return;
    const auto = skipKind === "intro" ? skipIntro.config.autoSkip : skipCredits.config.autoSkip;
    const key = `${slot.token}:${skipKind}`;
    if (!auto || autoSkipped.current.has(key)) return;
    autoSkipped.current.add(key);
    doSkip(skipKind);
  }, [doSkip, skipCredits.config.autoSkip, skipIntro.config.autoSkip, skipKind, slot]);

  useEffect(() => {
    if (!nextUp.enabled || !nextItem || nextUpDismissed || nextUpAt === null) return;
    if (time >= nextUpAt && nextUpShownAt === null) setNextUpShownAt(time);
    else if (time < nextUpAt - 1 && nextUpShownAt !== null) setNextUpShownAt(null);
  }, [nextItem, nextUp.enabled, nextUpAt, nextUpDismissed, nextUpShownAt, time]);

  const nextUpLeft =
    nextUp.enabled && nextItem && !nextUpDismissed && nextUpShownAt !== null
      ? nextUpRemaining(time, nextUpShownAt, countdownSeconds)
      : null;
  useEffect(() => {
    if (nextUpLeft === 0) void advance(1);
  }, [advance, nextUpLeft]);

  // ── Gapless preload ───────────────────────────────────────────────────────

  useEffect(() => {
    if (!gapless.enabled || !queue || !slot || !nextItem) return;
    const target = slot.index + 1;
    const other = (1 - active) as 0 | 1;
    if (slots[other]?.index === target || preloading.current === target || preloadFailed.current.has(target)) return;
    if (!shouldPreloadNext(time, duration, nextUpAt)) return;
    preloading.current = target;
    prepare(queue, target)
      .then((prepared) => {
        const still = slotsRef.current[activeRef.current];
        if ("slot" in prepared && still && still.index === target - 1) {
          setSlots((prev) => {
            const next: SlotPair = [...prev];
            next[(1 - activeRef.current) as 0 | 1] = prepared.slot;
            return next;
          });
        } else {
          preloadFailed.current.add(target);
        }
      })
      .catch(() => preloadFailed.current.add(target))
      .finally(() => {
        preloading.current = null;
      });
  }, [active, duration, gapless.enabled, nextItem, nextUpAt, prepare, queue, slot, slots, time]);

  // ── Element events ────────────────────────────────────────────────────────

  const onLoadedMetadata = (i: 0 | 1) => (event: SyntheticEvent<HTMLVideoElement>) => {
    const v = event.currentTarget;
    const s = slotsRef.current[i];
    if (!s) return;
    if (s.route.kind === "direct" && s.startAt > 0 && !startApplied.current.has(s.token)) {
      startApplied.current.add(s.token);
      v.currentTime = s.startAt;
    }
    if (i !== activeRef.current) return;
    setElementDuration(v.duration);
    pendingSeek.current = null;
    if (wantPlay.current) void v.play().catch((error) => console.warn("Playback did not start:", error));
  };

  const onPlaying = (i: 0 | 1) => (event: SyntheticEvent<HTMLVideoElement>) => {
    const s = slotsRef.current[i];
    if (i !== activeRef.current || !s) return;
    setBuffering(false);
    hold.current = null;
    if (started.current.has(s.token)) return;
    started.current.add(s.token);
    fadeIn(event.currentTarget, live.current.fade);
    if (s.resumedFrom !== null) setResumeNotice(s.resumedFrom);
    void recordActivity("playback.started", s.item.title, {
      mediaId: s.item.id ?? s.source.mediaId,
      route: s.route.kind,
      source: requestSource ?? "library",
      position: Math.round(s.offset + s.startAt),
    });
  };

  const onTimeUpdate = (i: 0 | 1) => (event: SyntheticEvent<HTMLVideoElement>) => {
    const s = slotsRef.current[i];
    if (i !== activeRef.current || !s || pendingSeek.current !== null) return;
    const at = s.offset + event.currentTarget.currentTime;
    setTime(at);
    if (live.current.autoResume && shouldSaveProgress(lastSave.current.at, Date.now(), lastSave.current.position, at)) {
      persist(s, event.currentTarget);
    }
  };

  const onPause = (i: 0 | 1) => (event: SyntheticEvent<HTMLVideoElement>) => {
    const s = slotsRef.current[i];
    if (i !== activeRef.current || !s) return;
    if (hold.current) return; // paused to refill the buffer, not by the viewer
    setPlaying(false);
    if (live.current.autoResume && !event.currentTarget.ended) persist(s, event.currentTarget);
  };

  const onWaiting = (i: 0 | 1) => (event: SyntheticEvent<HTMLVideoElement>) => {
    if (i !== activeRef.current) return;
    setBuffering(true);
    const v = event.currentTarget;
    if (live.current.plan.bufferAheadSeconds > 0 && !v.paused && !hold.current) {
      hold.current = { since: Date.now() };
      v.pause();
    }
  };

  const onEnded = (i: 0 | 1) => (event: SyntheticEvent<HTMLVideoElement>) => {
    const s = slotsRef.current[i];
    if (i !== activeRef.current || !s) return;
    const total = slotDuration(s, event.currentTarget);
    const at = s.offset + event.currentTarget.currentTime;
    if (s.route.kind === "transcode" && total > 0 && at < total - 15) {
      // The transcode stopped early (ffmpeg failed); keep the real position.
      addStatusMessage(`Playback of ${s.item.title} stopped early at ${formatClock(at)}`);
      if (live.current.autoResume) persist(s, event.currentTarget, at);
      void close();
      return;
    }
    persist(s, event.currentTarget, total);
    const list = queueRef.current;
    const hasNext = !!list && s.index + 1 < list.length;
    if (hasNext && (live.current.nextUp || live.current.gapless)) void advance(1, true);
    else void close();
  };

  const onError = (i: 0 | 1) => (event: SyntheticEvent<HTMLVideoElement>) => {
    const s = slotsRef.current[i];
    if (!s || !event.currentTarget.getAttribute("src")) return;
    if (i !== activeRef.current) {
      // A failed preload just means the next title loads normally later.
      preloadFailed.current.add(s.index);
      setSlots((prev) => {
        const next: SlotPair = [...prev];
        next[i] = null;
        return next;
      });
      return;
    }
    const fallback = routeAfterError(s.route, {
      filePath: s.source.filePath,
      probe: s.source.probe,
      forceDirectPlay: s.source.forceDirectPlay,
      transcodeAvailable: Boolean(s.source.transcodeUrl),
    });
    if (fallback.kind === "transcode" && s.source.transcodeUrl) {
      const at = Math.max(s.startAt, s.offset + event.currentTarget.currentTime);
      setSlots((prev) => {
        const next: SlotPair = [...prev];
        next[i] = { ...s, route: fallback, offset: at, startAt: 0, src: transcodeSrc(s.source.transcodeUrl!, at) };
        return next;
      });
      return;
    }
    void handOff(s.item, fallback.kind === "external" ? fallback.reason : `Couldn't play ${s.item.title}.`);
  };

  const elementEvents = (i: 0 | 1) => ({
    onLoadedMetadata: onLoadedMetadata(i),
    onPlaying: onPlaying(i),
    onPlay: () => i === activeRef.current && setPlaying(true),
    onPause: onPause(i),
    onTimeUpdate: onTimeUpdate(i),
    onWaiting: onWaiting(i),
    onEnded: onEnded(i),
    onError: onError(i),
    onDurationChange: (event: SyntheticEvent<HTMLVideoElement>) =>
      i === activeRef.current && setElementDuration(event.currentTarget.duration),
  });

  // Stream Buffering Control: after a stall, wait for the buffer target.
  useEffect(() => {
    if (!buffering) return;
    const timer = window.setInterval(() => {
      const v = activeVideo();
      const s = slotsRef.current[activeRef.current];
      if (!v || !s || !hold.current) return;
      const ahead = bufferedAhead(timeRanges(v.buffered), v.currentTime);
      const total = slotDuration(s, v);
      const remaining = total > 0 ? total - (s.offset + v.currentTime) : Number.POSITIVE_INFINITY;
      const loading = v.networkState === HTMLMediaElement.NETWORK_LOADING;
      const decision = bufferHold(ahead, live.current.plan.bufferAheadSeconds, remaining, Date.now() - hold.current.since, loading);
      if (decision === "resume") {
        hold.current = null;
        if (wantPlay.current) void v.play().catch(() => undefined);
      }
    }, 200);
    return () => window.clearInterval(timer);
  }, [activeVideo, buffering, slotDuration]);

  // Free a slot's network stream (and its ffmpeg process) when it is cleared.
  useEffect(() => {
    videos.forEach((ref, i) => {
      const v = ref.current;
      if (v && !slots[i] && v.currentSrc) {
        v.removeAttribute("src");
        v.load();
      }
    });
  }, [slots, videos]);

  useEffect(() => {
    videos.forEach((ref) => {
      if (!ref.current) return;
      ref.current.volume = volume;
      ref.current.muted = muted;
    });
  }, [muted, volume, slots, videos]);

  useEffect(() => {
    if (resumeNotice === null) return;
    const timer = window.setTimeout(() => setResumeNotice(null), 8000);
    return () => window.clearTimeout(timer);
  }, [resumeNotice]);

  useEffect(() => {
    const onChange = () => setFullscreen(Boolean(document.fullscreenElement));
    document.addEventListener("fullscreenchange", onChange);
    return () => document.removeEventListener("fullscreenchange", onChange);
  }, []);

  // ── Keyboard and focus ────────────────────────────────────────────────────

  const open = queue !== null;
  useEffect(() => {
    if (!open) return;
    containerRef.current?.focus();
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.closest("textarea, select, [contenteditable='true'], input:not([type='range'])")) return;
      const actions: Record<string, () => void> = {
        " ": togglePlay,
        ArrowLeft: () => jump(-10),
        ArrowRight: () => jump(10),
        f: toggleFullscreen,
        F: toggleFullscreen,
        m: () => setMuted((value) => !value),
        M: () => setMuted((value) => !value),
        Escape: () => {
          if (!document.fullscreenElement) void close();
        },
      };
      const action = actions[event.key];
      if (!action) return;
      event.preventDefault();
      event.stopPropagation();
      action();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [close, jump, open, toggleFullscreen, togglePlay]);

  // Cinema Mode hides the controls and cursor while the title plays untouched.
  const revealControls = useCallback(() => {
    setControlsVisible(true);
    window.clearTimeout(hideTimer.current);
    hideTimer.current = window.setTimeout(() => setControlsVisible(false), CONTROLS_HIDE_MS);
  }, []);
  useEffect(() => () => window.clearTimeout(hideTimer.current), []);
  const showChrome = !cinema.enabled || !playing || controlsVisible;

  if (!open) return null;

  const routeLabel = slot?.route.kind === "transcode" ? "Transcoding to H.264" : "Direct play";
  const title = slot ? `${slot.item.title}${slot.item.year ? ` (${slot.item.year})` : ""}` : "Loading";

  return (
    <div
      className={
        cinema.enabled
          ? "fixed inset-0 z-[300] flex items-center justify-center bg-black"
          : "fixed inset-0 z-[300] flex items-center justify-center bg-black/55 p-6 backdrop-blur-sm"
      }
      data-testid="library-player"
      data-cinema={cinema.enabled ? "on" : "off"}
    >
      <div
        ref={containerRef}
        role="dialog"
        aria-modal="true"
        aria-label={`Player: ${title}`}
        tabIndex={-1}
        onMouseMove={revealControls}
        className={`relative flex items-center justify-center overflow-hidden bg-black outline-none ${
          cinema.enabled
            ? `h-full w-full ${showChrome ? "" : "cursor-none"}`
            : "aspect-video w-[min(1100px,94vw)] max-h-[86vh] rounded-xl border border-white/10 shadow-2xl"
        }`}
      >
        {([0, 1] as const).map((i) => {
          const s = slots[i];
          return (
            <video
              key={i}
              ref={videos[i]}
              src={s?.src}
              preload={i === active ? plan.preload : "auto"}
              // CORS-clean media is required for the Web Audio crossfade; the asset
              // protocol and the loopback server both send CORS headers.
              crossOrigin="anonymous"
              playsInline
              onClick={i === active ? togglePlay : undefined}
              className={i === active ? "h-full w-full object-contain" : "hidden"}
              {...elementEvents(i)}
            />
          );
        })}

        {buffering && (
          <div className="pointer-events-none absolute inset-0 flex items-center justify-center" role="status" aria-label="Buffering">
            <Loader2 size={42} className="animate-spin text-white/80" />
          </div>
        )}

        <div
          className={`absolute inset-x-0 top-0 flex items-start gap-3 bg-gradient-to-b from-black/80 to-transparent p-4 transition-opacity ${
            showChrome ? "opacity-100" : "pointer-events-none opacity-0"
          }`}
        >
          <div className="min-w-0 flex-1">
            <div className="truncate text-sm font-semibold text-white">{title}</div>
            <div className="text-[11px] text-white/60">{routeLabel}</div>
          </div>
          <button
            type="button"
            onClick={() => void close()}
            aria-label="Close player"
            className="flex h-9 w-9 items-center justify-center rounded-lg text-white/85 hover:bg-white/10"
          >
            <X size={18} />
          </button>
        </div>

        {resumeNotice !== null && (
          <div className="absolute left-4 top-16 flex items-center gap-3 rounded-lg bg-black/75 px-3 py-2 text-xs text-white" role="status">
            <span>Resumed from {formatClock(resumeNotice)}</span>
            <button
              type="button"
              className="cv-btn-secondary px-2 py-1 text-xs"
              onClick={() => {
                setResumeNotice(null);
                seek(0);
              }}
            >
              Start over
            </button>
          </div>
        )}

        <div className="absolute bottom-24 right-4 flex flex-col items-end gap-2">
          {skipKind && (
            <button type="button" className="cv-btn px-4 py-2 text-sm" onClick={() => doSkip(skipKind)}>
              {skipKind === "intro" ? "Skip Intro" : "Skip Credits"}
            </button>
          )}
          {nextUpLeft !== null && nextItem && (
            <div className="w-72 rounded-xl border border-white/10 bg-black/85 p-3 text-xs text-white shadow-xl" role="status" aria-live="polite">
              <div className="text-[10px] uppercase tracking-wider text-white/60">Next up in {nextUpLeft}s</div>
              <div className="mt-1 truncate text-sm font-semibold">{nextItem.title}</div>
              <div className="mt-2 flex gap-2">
                <button type="button" className="cv-btn px-3 py-1 text-xs" onClick={() => void advance(1)}>
                  Play now
                </button>
                <button type="button" className="cv-btn-secondary px-3 py-1 text-xs" onClick={() => setNextUpDismissed(true)}>
                  Cancel
                </button>
              </div>
            </div>
          )}
        </div>

        <div
          className={`absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/85 to-transparent px-4 pb-3 pt-10 transition-opacity ${
            showChrome ? "opacity-100" : "pointer-events-none opacity-0"
          }`}
        >
          <PlayerControls
            playing={playing}
            time={time}
            duration={duration}
            volume={volume}
            muted={muted}
            fullscreen={fullscreen}
            hasPrevious={Boolean(previousItem)}
            hasNext={Boolean(nextItem)}
            onTogglePlay={togglePlay}
            onSeek={seek}
            onJump={jump}
            onVolume={(value) => {
              setVolume(value);
              setMuted(value === 0);
            }}
            onToggleMute={() => setMuted((value) => !value)}
            onToggleFullscreen={toggleFullscreen}
            onPrevious={() => void advance(-1)}
            onNext={() => void advance(1)}
          />
        </div>
      </div>
    </div>
  );
};

export default LibraryPlayer;
