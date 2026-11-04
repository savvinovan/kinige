import { useCallback, useEffect, useRef, useState, type ChangeEvent, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { formatDuration, UNKNOWN_TIME, wholeSeconds } from "../../voice/format";
import { FULL_MAX_BYTES, useMediaUrl } from "../attachments/mediaUrl";
import { FileCard } from "../attachments/FileCard";
import { Icon } from "../icons/Icon";
import { claimPlayback, currentSpeed, nextSpeed, releasePlayback, speedLabel, type PlayingHandle } from "./playerSession";
import "./voice-player.css";

/** The arrow keys move the position by this much. */
const SEEK_STEP_MS = 5000;

interface VoicePlayerProps {
  /** An `audio` attachment: a recorder's voice note or a Telegram voice or audio file. */
  attachment: AttachmentView;
  /** "Open in default app" on a file this web view cannot play. */
  onOpenFile: (attachmentId: string) => void;
}

/** Whether this web view says it can play the type; asked once per type. */
const playable = new Map<string, boolean>();
function canPlayHere(mime: string): boolean {
  const known = playable.get(mime);
  if (known !== undefined) return known;
  const answer = typeof document === "undefined" ? "maybe" : document.createElement("audio").canPlayType(mime);
  playable.set(mime, answer !== "");
  return answer !== "";
}

/** Tests only: forget what the web view said about types. */
export function resetPlayableCache(): void {
  playable.clear();
}

/**
 * A voice note in the bubble (slice 7a): a round play / pause button, a seek bar, "0:12 / 0:40" and the speed chip. The player has a fixed
 * width, so nothing moves when it loads. The file is requested only when Play is first pressed (opening a chat loads no audio). A file
 * that is gone shows the slice 7 tile; one this web view cannot play shows "Can't play here" with "Open in default app".
 */
export function VoicePlayer({ attachment, onOpenFile }: VoicePlayerProps) {
  const { t } = useTranslation();
  const mediaUrl = useMediaUrl();
  const audio = useRef<HTMLAudioElement>(null);
  const handle = useRef<PlayingHandle>({ pause: () => audio.current?.pause() });
  const seekOnStart = useRef<number | null>(null);
  const [started, setStarted] = useState(false);
  const [isPlaying, setIsPlaying] = useState(false);
  const [positionMs, setPositionMs] = useState(0);
  const [loadedMs, setLoadedMs] = useState<number | null>(null);
  const [failed, setFailed] = useState(false);
  const [speed, setSpeed] = useState(currentSpeed);

  // The player goes away while it plays (the chat closes): it stops, and the next player can claim the playback.
  useEffect(() => {
    const own = handle.current;
    return () => releasePlayback(own);
  }, []);

  const totalMs = attachment.durationMs ?? loadedMs;
  const total = totalMs === null ? UNKNOWN_TIME : formatDuration(totalMs);
  const unplayable = failed || attachment.byteSize > FULL_MAX_BYTES || !canPlayHere(attachment.mime);

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
    const clamped = Math.min(Math.max(0, ms), totalMs ?? Math.max(ms, 0));
    setPositionMs(clamped);
    const element = audio.current;
    if (!element || !element.src) {
      // Not started yet: the position is applied as soon as the file knows its length.
      seekOnStart.current = clamped / 1000;
      return;
    }
    element.currentTime = clamped / 1000;
  };

  const onSeek = (event: ChangeEvent<HTMLInputElement>) => seekTo(Number(event.target.value));

  const onSeekKey = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    seekTo(positionMs + (event.key === "ArrowRight" ? SEEK_STEP_MS : -SEEK_STEP_MS));
  };

  const cycleSpeed = () => {
    const next = nextSpeed();
    setSpeed(next);
    if (audio.current) audio.current.playbackRate = next;
  };

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
  const progress = max > 0 ? Math.min(100, (positionMs / max) * 100) : 0;
  const shownTime = started ? `${formatDuration(positionMs)} / ${total}` : total;

  return (
    <div className="voice-player" role="group" aria-label={label}>
      <button
        type="button"
        className="voice-player__play"
        aria-label={isPlaying ? t("voice.pause") : t("voice.play")}
        onClick={toggle}
      >
        <Icon name={isPlaying ? "pause" : "play"} />
      </button>
      <input
        type="range"
        className="voice-player__seek"
        aria-label={t("voice.seek")}
        aria-valuetext={`${formatDuration(positionMs)} / ${total}`}
        min={0}
        max={max}
        step={1000}
        value={Math.min(positionMs, max)}
        disabled={max === 0}
        style={{ "--voice-progress": `${progress}%` } as React.CSSProperties}
        onChange={onSeek}
        onKeyDown={onSeekKey}
      />
      <span className="voice-player__time">{shownTime}</span>
      <button
        type="button"
        className="voice-player__speed"
        aria-label={t("voice.speed", { speed: speedLabel(speed) })}
        onClick={cycleSpeed}
      >
        {speedLabel(speed)}
      </button>
      {/* Hidden, no `src` until the first Play. The element is the engine, the controls above are ours. */}
      <audio
        ref={audio}
        preload="none"
        onPlay={() => {
          setIsPlaying(true);
          claimPlayback(handle.current);
        }}
        onPause={() => {
          setIsPlaying(false);
          releasePlayback(handle.current);
        }}
        onEnded={() => {
          setIsPlaying(false);
          setPositionMs(0);
          releasePlayback(handle.current);
        }}
        onTimeUpdate={(event) => setPositionMs(Math.round(event.currentTarget.currentTime * 1000))}
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
