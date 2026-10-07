import { listen } from "@tauri-apps/api/event";
import React, { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import "./RecordingOverlay.css";
import { commands, events } from "@/bindings";
import type {
  StreamPhase,
  StreamPhaseEvent,
  StreamTextEvent,
  StreamWorkKind,
} from "@/bindings";
import i18n, { syncLanguageFromSettings } from "@/i18n";
import { getLanguageDirection } from "@/lib/utils/rtl";
import { getLanguageLabel } from "@/lib/constants/languages";

type OverlayState =
  | "recording"
  | "streaming"
  | "transcribing"
  | "processing"
  | "notice";

// Number of reactive bars in the waveform (the simple, smoothed style shared by
// every overlay form). Mic levels arrive as 16 FFT buckets; each bar shows the
// loudest bucket in its share of the spectrum.
const WAVE_BARS = 9;

// Bars are scaled to the recent peak so a quiet microphone still visibly moves.
// The floor keeps room noise from being amplified into full-height bars.
const PEAK_FLOOR = 0.12;
const PEAK_DECAY = 0.995;

const RecordingOverlay: React.FC = () => {
  const { t } = useTranslation();
  const [isVisible, setIsVisible] = useState(false);
  const [state, setState] = useState<OverlayState>("recording");
  // `Stream::play()` returning does not mean hardware callbacks are flowing.
  // Stay visually in an arming state until the backend processes the first
  // actual microphone sample chunk.
  const [captureReady, setCaptureReady] = useState(false);
  const [notice, setNotice] = useState({ kind: "", value: "" });
  const [caption, setCaption] = useState<{
    language: string;
    prompt: string | null;
  }>({ language: "", prompt: null });
  const peakRef = useRef(PEAK_FLOOR);
  const [levels, setLevels] = useState<number[]>(Array(WAVE_BARS).fill(0));
  const [streamText, setStreamText] = useState<StreamTextEvent>({
    committed: "",
    tentative: "",
  });
  const [phase, setPhase] = useState<StreamPhase>("listening");
  const [workKind, setWorkKind] = useState<StreamWorkKind>("transcribing");
  const [elapsed, setElapsed] = useState(0);
  // Bumped on each new streaming session so the Live card remounts fresh (replays
  // the pop-in, and never animates in from the previous panel's open size).
  const [session, setSession] = useState(0);
  // Overlay placement (top vs bottom of the screen). The Live panel grows downward
  // from a top overlay (oldest line under the pill) and upward from a bottom one.
  const [position, setPosition] = useState<"top" | "bottom">("bottom");
  // True once live text overflows the cap. A top overlay fades its top edge only
  // while overflowing, so the resting first line stays crisp flush under the pill.
  const [overflowing, setOverflowing] = useState(false);

  const smoothedLevelsRef = useRef<number[]>(Array(16).fill(0));
  // Live-text scroll-back: the text region "sticks" to the newest line while the
  // user is at the bottom; if they scroll up to read history, auto-follow pauses
  // until they scroll back down.
  const capRef = useRef<HTMLDivElement>(null);
  const pinnedRef = useRef(true);
  const direction = getLanguageDirection(i18n.language);

  useEffect(() => {
    const setupEventListeners = async () => {
      const unlistenShow = await listen("show-overlay", async (event) => {
        const overlayState = event.payload as OverlayState;
        // Reset synchronously before settings I/O. A fast microphone can emit
        // recording-ready while the awaits below are in flight; resetting after
        // them would overwrite that event and leave the overlay stuck arming.
        if (overlayState === "recording" || overlayState === "streaming") {
          setCaptureReady(false);
          smoothedLevelsRef.current = Array(16).fill(0);
          peakRef.current = PEAK_FLOOR;
          setLevels(Array(WAVE_BARS).fill(0));
          setStreamText({ committed: "", tentative: "" });
        }

        await syncLanguageFromSettings();
        // The Live panel flows downward from a top overlay and upward from a
        // bottom one; read the placement so the layout can flip to match.
        try {
          const settings = await commands.getAppSettings();
          if (settings.status === "ok") {
            setPosition(
              settings.data.overlay_position === "top" ? "top" : "bottom",
            );
          }
        } catch {
          // Keep the previous/default placement if settings can't be read.
        }
        setState(overlayState);
        if (overlayState === "streaming") {
          setPhase("listening");
          setWorkKind("transcribing");
          setElapsed(0);
          setSession((s) => s + 1); // remount the card fresh for this session
        }
        setIsVisible(true);
      });

      const unlistenNotice = await listen<{ kind: string; value: string }>(
        "overlay-notice",
        (event) => setNotice(event.payload),
      );

      const unlistenCaption = await listen<{
        language: string;
        prompt: string | null;
      }>("overlay-caption", (event) => setCaption(event.payload));

      const unlistenHide = await listen("hide-overlay", () => {
        setIsVisible(false);
        setCaptureReady(false);
      });

      const unlistenReady = await listen("recording-ready", () => {
        setElapsed(0);
        setCaptureReady(true);
      });

      const unlistenLevel = await listen<number[]>("mic-level", (event) => {
        const newLevels = event.payload as number[];
        // Track the recent peak and scale to it, so any speech clearly moves the bars.
        const frameMax = Math.max(0, ...newLevels);
        peakRef.current = Math.max(
          PEAK_FLOOR,
          frameMax,
          peakRef.current * PEAK_DECAY,
        );
        const gain = 0.8 / peakRef.current;
        // Exponential smoothing across the 16 buckets.
        const smoothed = smoothedLevelsRef.current.map((prev, i) => {
          const target = Math.min(1, (newLevels[i] || 0) * gain);
          return prev * 0.7 + target * 0.3;
        });
        smoothedLevelsRef.current = smoothed;
        // Each bar shows the loudest bucket in its slice of the spectrum.
        setLevels(
          Array.from({ length: WAVE_BARS }, (_, i) => {
            const from = Math.floor((i * smoothed.length) / WAVE_BARS);
            const to = Math.floor(((i + 1) * smoothed.length) / WAVE_BARS);
            return Math.max(...smoothed.slice(from, Math.max(to, from + 1)));
          }),
        );
      });

      const unlistenStream = await events.streamTextEvent.listen((event) => {
        setStreamText(event.payload);
      });

      const unlistenPhase = await events.streamPhaseEvent.listen((event) => {
        const payload: StreamPhaseEvent = event.payload;
        setPhase(payload.phase);
        if (payload.kind) setWorkKind(payload.kind);
      });

      return () => {
        unlistenShow();
        unlistenNotice();
        unlistenCaption();
        unlistenHide();
        unlistenReady();
        unlistenLevel();
        unlistenStream();
        unlistenPhase();
      };
    };

    setupEventListeners();
  }, []);

  // Elapsed capture timer starts only once microphone samples are flowing.
  useEffect(() => {
    if (state !== "streaming" || !isVisible || !captureReady) return;
    const id = setInterval(() => setElapsed((e) => e + 1), 1000);
    return () => clearInterval(id);
  }, [state, isVisible, captureReady]);

  // Stick to the bottom as text streams in — but only while pinned, so a user who
  // has scrolled up to read history isn't yanked back down by the next chunk.
  useLayoutEffect(() => {
    const el = capRef.current;
    if (!el) return;
    // Fade the top edge only once text actually overflows the cap.
    setOverflowing(el.scrollHeight > el.clientHeight + 1);
    if (pinnedRef.current) el.scrollTop = el.scrollHeight;
  }, [streamText]);

  // Each fresh streaming session starts pinned to the bottom, fade cleared.
  useEffect(() => {
    pinnedRef.current = true;
    setOverflowing(false);
  }, [session]);

  if (!isVisible) return null;

  // Re-pin when the user is within ~a line of the bottom; unpin otherwise.
  const handleStreamScroll = () => {
    const el = capRef.current;
    if (!el) return;
    pinnedRef.current = el.scrollHeight - el.scrollTop - el.clientHeight <= 16;
  };

  const fmtTime = (s: number) =>
    `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;

  // ---- Shared building blocks (one visual language for every overlay form) ----
  const waveform = (
    <div className={`swave ${captureReady ? "ready" : "arming"}`}>
      {levels.map((v, i) => (
        <i
          key={i}
          style={{
            height: `${Math.max(3, Math.min(18, 3 + Math.pow(v, 0.7) * 15))}px`,
          }}
        />
      ))}
    </div>
  );

  const cancelBtn = (
    <button
      className="sx"
      aria-label="cancel"
      onClick={() => commands.cancelOperation()}
    >
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <path
          d="M4 4 L12 12 M12 4 L4 12"
          stroke="currentColor"
          strokeWidth="1.6"
          strokeLinecap="round"
        />
      </svg>
    </button>
  );

  // dot (left) | waveform (center) | timer + cancel (right) — same structure for
  // pill & panel, so the Live morph is a pure width change.
  const listeningRow = (showTimer: boolean, showCancel: boolean) => (
    <div className="sbase">
      <div className="sbase-l">
        <span className={`sdot ${captureReady ? "ready" : "arming"}`} />
      </div>
      {waveform}
      <div className="sbase-r">
        {showTimer && <span className="stimer">{fmtTime(elapsed)}</span>}
        {showCancel && cancelBtn}
      </div>
    </div>
  );

  const languageName = (code: string) =>
    getLanguageLabel(code) ??
    (code === "auto" ? t("settings.general.language.auto") : code);

  // Language (and prompt, when post-processing will run) for this recording.
  const captionText = [
    caption.language ? languageName(caption.language) : "",
    caption.prompt ?? "",
  ]
    .filter(Boolean)
    .join(" · ");
  const captionRow = captionText ? (
    <div className="scap">{captionText}</div>
  ) : null;

  // spinner (left) | label (center) | cancel (right) — same 3-zone grid as the
  // listening row, so the label is centered.
  const workingRow = (label: string, showCancel: boolean) => (
    <div className="sbase">
      <div className="sbase-l">
        <span className="sspinner" />
      </div>
      <span className="swork-label">{label}</span>
      <div className="sbase-r">{showCancel && cancelBtn}</div>
    </div>
  );

  // ---- Live overlay: a pill that sculpts open into a panel ----
  if (state === "streaming") {
    const hasText =
      streamText.committed.length > 0 || streamText.tentative.length > 0;
    const working = phase === "working";
    // Keep the panel open whenever there's text — even while finalizing — so the
    // transcript stays put under a working spinner instead of collapsing and
    // squishing the text mid-stream. Only fall back to the small working pill
    // when there was no text to preserve.
    const open = hasText;
    const collapsed = working && !hasText;

    return (
      <div dir={direction} className={`ov-stage ${position}`}>
        <div
          key={session}
          className={`scard ${open ? "open" : ""} ${collapsed ? "working" : ""} ${
            isVisible ? "" : "leaving"
          }`}
        >
          <div className="stext">
            <div className="stext-clip">
              <div
                className={`stext-cap ${overflowing ? "overflowing" : ""}`}
                ref={capRef}
                onScroll={handleStreamScroll}
              >
                <p>
                  <span className="committed">
                    {streamText.committed ? streamText.committed + " " : ""}
                  </span>
                  <span className="tentative">{streamText.tentative}</span>
                  {/* Drop the blinking caret once finalizing — it's no longer
                      capturing, and a static spinner conveys the work. */}
                  {!working && <span className="scaret" />}
                </p>
              </div>
            </div>
          </div>
          {working
            ? workingRow(
                workKind === "polishing"
                  ? t("overlay.processing")
                  : t("overlay.transcribing"),
                true,
              )
            : listeningRow(open, true)}
          {!working && captionRow}
        </div>
      </div>
    );
  }

  // ---- Minimal overlay: exactly one row at a time — waveform (recording), or a
  // spinner + label (transcribing / processing). Never both. The pill animates its
  // width between them; the cancel button is in both rows so it stays put.
  const working = state === "transcribing" || state === "processing";
  const showsNotice = state === "notice";
  const noticeValue =
    notice.kind === "language" ? languageName(notice.value) : notice.value;
  const noticeText = t(
    notice.kind === "language"
      ? "overlay.noticeLanguage"
      : notice.kind === "vocab"
        ? "overlay.noticeVocab"
        : notice.kind === "learned"
          ? "overlay.noticeLearned"
          : notice.kind === "learned-none"
            ? "overlay.noticeLearnedNone"
            : notice.kind === "meeting"
              ? "overlay.noticeMeeting"
              : notice.kind === "model"
                ? "overlay.noticeModel"
                : notice.kind === "model-failed"
                  ? "overlay.noticeModelFailed"
                  : notice.kind === "journal"
                    ? "overlay.noticeJournal"
                    : notice.kind === "journal-failed"
                      ? "overlay.noticeJournalFailed"
                      : notice.kind === "llm"
                        ? "overlay.noticeLlm"
                        : notice.kind === "llm-failed"
                          ? "overlay.noticeLlmFailed"
                          : notice.kind === "paste-moved"
                            ? "overlay.noticePasteMoved"
                            : notice.kind === "scratched"
                              ? "overlay.noticeScratched"
                              : notice.kind === "transform-moved"
                                ? "overlay.noticeNotAtCursor"
                                : notice.kind === "reformat-none"
                                  ? "overlay.noticeNoSelection"
                                  : notice.kind === "reformat-setup"
                                    ? "overlay.noticeReformatSetup"
                                    : notice.kind === "reformat-failed"
                                      ? "overlay.noticeReformatFailed"
                                      : notice.kind === "synced"
                                        ? "overlay.noticeSynced"
                                        : notice.kind === "synced-failed"
                                          ? "overlay.noticeSyncFailed"
                                          : notice.kind === "fallback"
                                            ? "overlay.noticeFallback"
                                            : "overlay.noticePrompt",
    { value: noticeValue },
  );
  const workLabel =
    state === "processing"
      ? t("overlay.processing")
      : t("overlay.transcribing");

  return (
    <div
      dir={direction}
      className={`ov-stage ${position} ov-fade ${isVisible ? "show" : ""}`}
    >
      <div
        className={`scard compact ${(working || showsNotice) && isVisible ? "cworking" : ""}`}
      >
        {showsNotice ? (
          <div className="sbase">
            <div className="sbase-l" />
            <span className="swork-label">{noticeText}</span>
            <div className="sbase-r" />
          </div>
        ) : working ? (
          workingRow(workLabel, true)
        ) : (
          <>
            {listeningRow(false, true)}
            {captionRow}
          </>
        )}
      </div>
    </div>
  );
};

export default RecordingOverlay;
