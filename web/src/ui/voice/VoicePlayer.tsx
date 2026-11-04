import {
  memo,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type CSSProperties,
  type KeyboardEvent,
  type PointerEvent,
} from "react";
import { useTranslation } from "react-i18next";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { useLanguage } from "../../i18n/I18n";
import { formatDuration, UNKNOWN_TIME, wholeSeconds } from "../../voice/format";
import { useWaveform } from "../../voice/waveformSource";
import { FULL_MAX_BYTES, useMediaUrl } from "../attachments/mediaUrl";
import { FileCard } from "../attachments/FileCard";
import { formatSize } from "../attachments/formatSize";
import { Icon } from "../icons/Icon";
import { claimPlayback, currentSpeed, nextSpeed, releasePlayback, speedLabel, subscribeSpeed, type PlayingHandle } from "./playerSession";
import "./voice-player.css";

/** The arrow keys move the position by this much. */
const SEEK_STEP_MS = 5000;
/** How many bars the waveform has (the number of peaks, `kinige_domain::attachment::WAVEFORM_PEAKS`). */
const BARS = 64;

interface VoicePlayerProps {
  /** An `audio` attachment: a recorder's voice note or a Telegram voice or audio file. */
  attachment: AttachmentView;
  /** "Open in default app" on a file this web view cannot play. */
  onOpenFile: (attachmentId: string) => void;
  /** Storybook only: shows the player as if it were playing at this position, without a media element doing it. */
  preview?: { playing: boolean; positionMs: number };
}

/** Whether this web view says it can play the type; asked once per type. */
const playable = new Map<string, boolean>();
function canPlayHere(mime: string): boolean {
  const known = playable.get(mime);
  if (known !== undefined) return known;
  const element = typeof document === "undefined" ? null : document.createElement("audio");
  // WebKit says "" for plain `audio/ogg` (it means Vorbis) but plays Opus in Ogg, which is what Telegram voice messages are.
  const candidates = mime === "audio/ogg" || mime === "audio/opus" ? [mime, "audio/ogg; codecs=opus"] : [mime];
  const ok = element === null || candidates.some((type) => element.canPlayType(type) !== "");
  playable.set(mime, ok);
  return ok;
}

/** Tests only: forget what the web view said about types. */
export function resetPlayableCache(): void {
  playable.clear();
}

/** Whether the player of this attachment is the waveform (and not the "File is missing" tile or "Can't play here"). */
export function showsWaveform(attachment: AttachmentView): boolean {
  return attachment.kind === "audio" && attachment.state !== "missing" && attachment.byteSize <= FULL_MAX_BYTES && canPlayHere(attachment.mime);
}

/** The bars of a placeholder: a fixed gentle shape, until the note's own peaks are known (the bubble does not change size). */
const PLACEHOLDER = Array.from({ length: BARS }, (_, i) => 0.28 + 0.16 * Math.sin(i * 0.6) + 0.08 * Math.sin(i * 1.7));

/** One layer of bars; the height of each is a unitless `--peak` the stylesheet turns into a length from the tokens. */
const Bars = memo(function Bars({ peaks }: { peaks: Uint8Array | null }) {
  const shape = useMemo(() => {
    const filled = peaks !== null && peaks.length === BARS;
    return Array.from({ length: BARS }, (_, i) => (filled ? (peaks[i] ?? 0) / 255 : (PLACEHOLDER[i] ?? 0)));
  }, [peaks]);
  return (
    <>
      {shape.map((peak, i) => (
        <span key={i} className="voice-player__bar" style={{ "--peak": peak.toFixed(3) } as CSSProperties} />
      ))}
    </>
  );
});

/**
 * A voice note in the bubble (slice 7a, round 2): a big round accent play / pause button and, to its right, a waveform of thin rounded
 * bars (played bars accent, the rest muted), with "0:40, 20.7 KB" under it (the elapsed time while playing). Clicking or dragging on the
 * waveform seeks; the arrow keys move by 5 s. While playing the fill follows `audio.currentTime` on every animation frame, written
 * straight into a CSS variable, so it moves smoothly (the media element's `timeupdate` fires about four times a second) and nothing but
 * the player ever re-renders. The file is requested only when Play is first pressed. A file that is gone shows the slice 7 tile; one this
 * web view cannot play shows "Can't play here" with "Open in default app". The bubble's time sits at the bottom right, on the meta line.
 */
export function VoicePlayer({ attachment, onOpenFile, preview }: VoicePlayerProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  const mediaUrl = useMediaUrl();
  const audio = useRef<HTMLAudioElement>(null);
  const wave = useRef<HTMLDivElement>(null);
  const handle = useRef<PlayingHandle>({ pause: () => audio.current?.pause() });
  const seekOnStart = useRef<number | null>(null);
  /** The position in milliseconds as the bars show it, finer than `positionMs` (which changes once a second while playing). */
  const exactMs = useRef(preview?.positionMs ?? 0);
  const shownSecond = useRef(-1);
  const dragging = useRef(false);
  const [started, setStarted] = useState(preview !== undefined);
  const [isPlaying, setIsPlaying] = useState(preview?.playing ?? false);
  const [positionMs, setPositionMs] = useState(preview?.positionMs ?? 0);
  const [loadedMs, setLoadedMs] = useState<number | null>(null);
  const [failed, setFailed] = useState(false);
  // One speed for every voice note of the session: the chip of any player shows it and moves it for all of them.
  const speed = useSyncExternalStore(subscribeSpeed, currentSpeed);

  const unplayable = failed || !showsWaveform(attachment);
  const peaks = useWaveform(attachment, wave, !unplayable && attachment.state !== "missing");
  const totalMs = attachment.durationMs ?? loadedMs;
  const totalRef = useRef(totalMs);
  const isPreview = preview !== undefined;
  const total = totalMs === null ? UNKNOWN_TIME : formatDuration(totalMs);

  // The player goes away while it plays (the chat closes): it stops, and the next player can claim the playback.
  useEffect(() => {
    const own = handle.current;
    return () => releasePlayback(own);
  }, []);

  /** Draws the fill at `ms`: one CSS variable on the waveform, no render. */
  const paint = useCallback((ms: number) => {
    const max = totalRef.current ?? 0;
    const fraction = max > 0 ? Math.min(1, Math.max(0, ms / max)) : 0;
    wave.current?.style.setProperty("--voice-progress", `${(fraction * 100).toFixed(2)}%`);
  }, []);

  /** Takes a position as the truth: the bars, and the text and the slider value once the whole second changes (or when forced). */
  const settle = useCallback(
    (ms: number, force = false) => {
      exactMs.current = ms;
      paint(ms);
      const second = Math.floor(ms / 1000);
      if (force || second !== shownSecond.current) {
        shownSecond.current = second;
        setPositionMs(Math.round(ms));
      }
    },
    [paint],
  );

  // The bars follow the total once it is known and when the player first draws.
  useLayoutEffect(() => {
    totalRef.current = totalMs;
    paint(exactMs.current);
  }, [paint, totalMs]);

  // While playing: one animation frame after another reads `currentTime`. It stops on pause, when the page is hidden and on unmount.
  useEffect(() => {
    if (!isPlaying || isPreview) return;
    let frame = 0;
    const step = () => {
      const element = audio.current;
      if (element && !dragging.current) settle(element.currentTime * 1000);
      frame = requestAnimationFrame(step);
    };
    const onVisibility = () => {
      cancelAnimationFrame(frame);
      if (!document.hidden) frame = requestAnimationFrame(step);
    };
    document.addEventListener("visibilitychange", onVisibility);
    if (!document.hidden) frame = requestAnimationFrame(step);
    return () => {
      cancelAnimationFrame(frame);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [isPlaying, isPreview, settle]);

  const play = useCallback(() => {
    const element = audio.current;
    if (!element) return;
    if (!element.src) {
      // The first Play: only now the file is asked for.
      element.src = mediaUrl("audio", attachment.id);
      setStarted(true);
    }
    element.playbackRate = speed;
    claimPlayback(handle.current);
    element.play().catch(() => {
      // Autoplay refused or the file failed: the person sees "Can't play here" only for a real load error (`onError` below).
      setIsPlaying(false);
      releasePlayback(handle.current);
    });
  }, [mediaUrl, attachment.id, speed]);

  const toggle = () => {
    const element = audio.current;
    if (!element) return;
    if (element.paused) play();
    else element.pause();
  };

  const seekTo = (ms: number) => {
    const max = totalMs ?? Math.max(ms, 0);
    const clamped = Math.min(Math.max(0, ms), max);
    settle(clamped, true);
    const element = audio.current;
    if (!element || !element.src) {
      // Not started yet: the position is applied as soon as the file knows its length.
      seekOnStart.current = clamped / 1000;
      return;
    }
    element.currentTime = clamped / 1000;
  };

  /** The position under a pointer, as a share of the waveform's width. */
  const msAt = (clientX: number): number => {
    const box = wave.current?.getBoundingClientRect();
    if (!box || box.width <= 0 || !totalMs) return 0;
    return Math.min(1, Math.max(0, (clientX - box.left) / box.width)) * totalMs;
  };

  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || !totalMs) return;
    try {
      // The drag goes on outside the waveform too, until the button is released.
      event.currentTarget.setPointerCapture(event.pointerId);
    } catch {
      // An environment without pointer capture (a test): the drag still works over the waveform.
    }
    dragging.current = true;
    settle(msAt(event.clientX), true);
  };
  const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    if (dragging.current) settle(msAt(event.clientX), true);
  };
  const onPointerEnd = (event: PointerEvent<HTMLDivElement>) => {
    if (!dragging.current) return;
    dragging.current = false;
    seekTo(msAt(event.clientX));
  };

  const onKey = (event: KeyboardEvent<HTMLDivElement>) => {
    const step = event.key === "ArrowRight" ? SEEK_STEP_MS : event.key === "ArrowLeft" ? -SEEK_STEP_MS : 0;
    if (step !== 0) seekTo(exactMs.current + step);
    else if (event.key === "Home") seekTo(0);
    else if (event.key === "End" && totalMs) seekTo(totalMs);
    else return;
    event.preventDefault();
  };

  useEffect(() => {
    if (audio.current) audio.current.playbackRate = speed;
  }, [speed]);

  if (attachment.state === "missing") return <FileCard attachment={attachment} onOpen={onOpenFile} />;

  if (unplayable) {
    return (
      <div className="voice-player voice-player--unplayable" role="group" aria-label={t("voice.note")}>
        <Icon name="audio" />
        <span className="voice-player__cant">{t("voice.cantPlay")}</span>
        <button type="button" className="voice-player__open" onClick={() => onOpenFile(attachment.id)}>
          {t("voice.openDefault")}
        </button>
      </div>
    );
  }

  const label =
    attachment.durationMs === null
      ? t("voice.noteLabelUnknown")
      : t("voice.noteLabel", { count: wholeSeconds(attachment.durationMs) });
  const max = totalMs ?? 0;
  // The total at rest, the elapsed time once the note plays or has been moved.
  const time = isPlaying || positionMs > 0 ? formatDuration(positionMs) : total;
  const meta = t("voice.meta", { time, size: formatSize(attachment.byteSize, language) });

  return (
    <div className="voice-player" role="group" aria-label={label}>
      <button
        type="button"
        className={isPlaying ? "voice-player__play" : "voice-player__play voice-player__play--idle"}
        aria-label={isPlaying ? t("voice.pause") : t("voice.play")}
        onClick={toggle}
      >
        <Icon name={isPlaying ? "pause" : "play"} />
      </button>
      <div
        ref={wave}
        className="voice-player__wave"
        role="slider"
        tabIndex={max > 0 ? 0 : -1}
        aria-label={t("voice.seek")}
        aria-valuemin={0}
        aria-valuemax={max}
        aria-valuenow={Math.min(positionMs, max)}
        aria-valuetext={t("voice.position", { time: formatDuration(positionMs), total })}
        aria-disabled={max === 0 || undefined}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerEnd}
        onPointerCancel={onPointerEnd}
        onKeyDown={onKey}
      >
        <span className="voice-player__bars" aria-hidden="true">
          <Bars peaks={peaks} />
        </span>
        <span className="voice-player__bars voice-player__bars--played" aria-hidden="true">
          <Bars peaks={peaks} />
        </span>
      </div>
      <div className="voice-player__meta">
        <span className="voice-player__time">{meta}</span>
        {started ? (
          <button
            type="button"
            className="voice-player__speed"
            aria-label={t("voice.speed", { speed: speedLabel(speed) })}
            onClick={nextSpeed}
          >
            {speedLabel(speed)}
          </button>
        ) : null}
      </div>
      {/* Hidden, no `src` until the first Play. The element is the engine, the controls above are ours. */}
      <audio
        ref={audio}
        preload="none"
        onPlay={() => {
          setIsPlaying(true);
          claimPlayback(handle.current);
        }}
        onPause={(event) => {
          setIsPlaying(false);
          releasePlayback(handle.current);
          settle(event.currentTarget.currentTime * 1000, true);
        }}
        onEnded={() => {
          setIsPlaying(false);
          settle(0, true);
          releasePlayback(handle.current);
        }}
        onTimeUpdate={(event) => {
          // The animation frames do this while playing; this covers a seek while paused and a hidden page.
          if (!dragging.current) settle(event.currentTarget.currentTime * 1000);
        }}
        onLoadedMetadata={(event) => {
          const element = event.currentTarget;
          if (Number.isFinite(element.duration)) setLoadedMs(Math.round(element.duration * 1000));
          if (seekOnStart.current !== null) {
            element.currentTime = seekOnStart.current;
            seekOnStart.current = null;
          }
        }}
        onError={() => {
          // Only an error of a file we asked for: the element has no `src` before the first Play.
          if (audio.current?.src) {
            setFailed(true);
            setIsPlaying(false);
          }
        }}
      />
    </div>
  );
}
