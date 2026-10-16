import { useEffect, useRef, useState } from "react";
import {
  ActionIcon,
  Alert,
  Button,
  Group,
  Loader,
  Select,
  Slider,
  Text,
  Tooltip,
} from "@mantine/core";
import {
  ArrowClockwiseIcon,
  FileVideoIcon,
  PauseIcon,
  PlayIcon,
  RepeatIcon,
  SpeakerHighIcon,
  SpeakerSlashIcon,
} from "@phosphor-icons/react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type { Guidance } from "./model";
import type { Strings } from "./components";

export async function checkImportedVideo(guidance: Guidance) {
  const video = document.createElement("video");
  video.muted = true;
  video.preload = "auto";
  let timeout: ReturnType<typeof setTimeout> | undefined;
  try {
    await new Promise<void>((resolve, reject) => {
      video.addEventListener(
        "loadeddata",
        () => (video.videoWidth ? resolve() : reject("unsupported-video")),
        { once: true },
      );
      video.addEventListener(
        "error",
        () =>
          reject(
            video.error?.code === 4 ? "unsupported-video" : "import-failed",
          ),
        { once: true },
      );
      timeout = setTimeout(() => reject("import-failed"), 12000);
      video.src = convertFileSrc(guidance.source.id, "guidance");
      video.load();
    });
  } finally {
    clearTimeout(timeout);
    video.pause();
    video.removeAttribute("src");
    video.load();
  }
}
export function GuidancePicker({
  value,
  onChange,
  onBusy,
  S,
  error,
  field,
}: {
  value?: Guidance | null;
  onChange: (value: Guidance | null) => void;
  onBusy: (delta: number) => void;
  S: Strings;
  error?: string;
  field?: string;
}) {
  const [importing, setImporting] = useState(false),
    [failure, setFailure] = useState("");
  const mounted = useRef(true),
    busy = useRef(false);
  const busyHandler = useRef(onBusy);
  const changeHandler = useRef(onChange);
  busyHandler.current = onBusy;
  changeHandler.current = onChange;
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      if (busy.current) {
        busy.current = false;
        busyHandler.current(-1);
      }
    };
  }, []);
  async function importFile() {
    if (busy.current) return;
    busy.current = true;
    onBusy(1);
    setImporting(true);
    setFailure("");
    try {
      const imported = await invoke<Guidance | null>("import_guidance_video");
      if (imported) {
        await checkImportedVideo(imported);
        if (mounted.current) changeHandler.current(imported);
      }
    } catch (cause) {
      if (mounted.current)
        setFailure(
          cause === "unsupported-video"
            ? S.unsupportedVideo
            : S.importVideoFailed,
        );
    } finally {
      if (busy.current) {
        busy.current = false;
        busyHandler.current(-1);
      }
      if (mounted.current) setImporting(false);
    }
  }
  return (
    <div className="guidance-picker" data-guidance-options="none,local-video">
      <Select
        label={S.guidance}
        value={value || importing ? "local-video" : ""}
        disabled={importing}
        error={error}
        data-field={field}
        data={[
          { value: "", label: S.noGuidance },
          { value: "local-video", label: S.localVideo },
        ]}
        onChange={(next) => {
          if (next === "local-video") void importFile();
          else {
            onChange(null);
            setFailure("");
          }
        }}
      />
      {(value || importing) && (
        <Group gap={10} className="guidance-file-row">
          <FileVideoIcon size={18} />
          <Text size="xs" c="dimmed">
            {importing ? S.importingVideo : S.videoSelected}
          </Text>
          <Button
            size="compact-xs"
            variant="subtle"
            loading={importing}
            onClick={() => void importFile()}
          >
            {S.chooseLocalVideo}
          </Button>
        </Group>
      )}
      {value && (
        <Text size="xs" c="dimmed" className="field-hint">
          {S.localVideoHint}
        </Text>
      )}
      {failure && (
        <Alert color="red" role="alert" variant="light">
          {failure}
        </Alert>
      )}
    </div>
  );
}
function videoTime(seconds: number) {
  return Number.isFinite(seconds)
    ? `${Math.floor(seconds / 60)}:${String(Math.floor(seconds % 60)).padStart(2, "0")}`
    : "0:00";
}
/** 播放器的生命周期只由资源引用及显式重试决定，tick 和主题不会替换 video 节点。 */
export function GuidanceVideo({
  guidance,
  S,
}: {
  guidance: Guidance;
  S: Strings;
}) {
  const ref = useRef<HTMLVideoElement>(null);
  const [retry, setRetry] = useState(0),
    [status, setStatus] = useState<
      "loading" | "ready" | "autoplay" | "error" | "timeout"
    >("loading");
  const [paused, setPaused] = useState(false),
    [muted, setMuted] = useState(true),
    [loop, setLoop] = useState(true);
  const [duration, setDuration] = useState(0),
    [position, setPosition] = useState(0);
  const seeking = useRef(false);
  useEffect(() => {
    const video = ref.current!;
    let disposed = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    setStatus("loading");
    setPosition(0);
    setDuration(0);
    function wait() {
      if (timer !== undefined) return;
      setStatus("loading");
      timer = setTimeout(() => {
        if (!disposed) {
          video.pause();
          setStatus("timeout");
        }
      }, 12000);
    }
    function ready() {
      clearTimeout(timer);
      timer = undefined;
      setStatus("ready");
    }
    function failed() {
      clearTimeout(timer);
      timer = undefined;
      setStatus("error");
    }
    function progress() {
      if (!seeking.current) setPosition(video.currentTime);
    }
    function metadata() {
      setDuration(Number.isFinite(video.duration) ? video.duration : 0);
    }
    function playback() {
      setPaused(video.paused);
    }
    function volume() {
      setMuted(video.muted);
    }
    const events: [string, () => void][] = [
      ["loadeddata", ready],
      ["playing", ready],
      ["error", failed],
      ["timeupdate", progress],
      ["durationchange", metadata],
      ["loadedmetadata", metadata],
      ["play", playback],
      ["pause", playback],
      ["volumechange", volume],
      ["waiting", wait],
      ["stalled", wait],
    ];
    events.forEach(([event, callback]) =>
      video.addEventListener(event, callback),
    );
    video.muted = video.defaultMuted = true;
    video.loop = true;
    setMuted(true);
    setLoop(true);
    if (/^local-[a-f0-9]{32}$/i.test(guidance.source.id)) {
      video.src = convertFileSrc(guidance.source.id, "guidance");
      video.load();
      wait();
      void video.play().catch(() => {
        if (!disposed && !video.error) {
          clearTimeout(timer);
          timer = undefined;
          setStatus("autoplay");
          setPaused(true);
        }
      });
    } else failed();
    function release() {
      video.pause();
      video.removeAttribute("src");
      video.load();
    }
    window.addEventListener("pagehide", release);
    return () => {
      disposed = true;
      clearTimeout(timer);
      events.forEach(([event, callback]) =>
        video.removeEventListener(event, callback),
      );
      window.removeEventListener("pagehide", release);
      release();
    };
  }, [guidance.source.id, retry]);
  async function toggle() {
    const video = ref.current!;
    if (!video.paused) video.pause();
    else
      try {
        await video.play();
        setStatus("ready");
      } catch {
        setStatus("autoplay");
      }
  }
  const unavailable = status === "error" || status === "timeout";
  return (
    <div className="video-player" data-testid="guidance-player">
      <video
        ref={ref}
        autoPlay
        muted
        loop
        playsInline
        preload="metadata"
        aria-label={S.videoGuidance}
      />
      {status !== "ready" && (
        <div
          className={`video-status${unavailable ? " failed" : ""}`}
          role="status"
        >
          {status === "loading" ? (
            <Loader size={22} color="white" />
          ) : (
            <>
              <Text size="sm">
                {status === "autoplay"
                  ? S.videoAutoplay
                  : status === "timeout"
                    ? S.videoTimeout
                    : S.videoFailed}
              </Text>
              <Button
                size="compact-sm"
                variant="white"
                leftSection={
                  status === "autoplay" ? (
                    <PlayIcon size={16} />
                  ) : (
                    <ArrowClockwiseIcon size={16} />
                  )
                }
                onClick={() =>
                  status === "autoplay" ? void toggle() : setRetry((n) => n + 1)
                }
              >
                {status === "autoplay" ? S.playVideo : S.retryGuidance}
              </Button>
            </>
          )}
        </div>
      )}
      <div className="video-controls">
        <ActionIcon
          data-action="toggle-playback"
          color="gray"
          variant="transparent"
          c="white"
          aria-label={paused ? S.play : S.pauseVideo}
          disabled={unavailable}
          onClick={() => void toggle()}
        >
          {paused ? (
            <PlayIcon size={18} weight="fill" />
          ) : (
            <PauseIcon size={18} weight="fill" />
          )}
        </ActionIcon>
        <Slider
          className="video-seek"
          value={Math.min(position, duration)}
          min={0}
          max={duration || 1}
          step={0.1}
          disabled={!duration || unavailable}
          label={null}
          thumbLabel={S.videoProgress}
          size="xs"
          onChange={(value) => {
            seeking.current = true;
            setPosition(value);
          }}
          onChangeEnd={(value) => {
            if (ref.current && duration) ref.current.currentTime = value;
            seeking.current = false;
          }}
        />
        <Text size="xs" className="video-time">
          {videoTime(position)} / {videoTime(duration)}
        </Text>
        <Tooltip label={muted ? S.videoUnmuted : S.videoMuted}>
          <ActionIcon
            variant="transparent"
            c="white"
            aria-label={muted ? S.videoUnmuted : S.videoMuted}
            onClick={() => {
              if (ref.current) ref.current.muted = !ref.current.muted;
            }}
          >
            {muted ? (
              <SpeakerSlashIcon size={18} />
            ) : (
              <SpeakerHighIcon size={18} />
            )}
          </ActionIcon>
        </Tooltip>
        <Tooltip label={S.toggleLoop}>
          <ActionIcon
            variant="transparent"
            c={loop ? "plum.3" : "gray.5"}
            aria-label={S.toggleLoop}
            aria-pressed={loop}
            onClick={() => {
              if (ref.current) {
                ref.current.loop = !loop;
                setLoop(!loop);
              }
            }}
          >
            <RepeatIcon size={18} />
          </ActionIcon>
        </Tooltip>
      </div>
    </div>
  );
}
