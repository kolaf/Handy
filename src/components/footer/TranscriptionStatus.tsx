import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { events, type RecordingPhase, type StreamPhaseEvent } from "@/bindings";
import { useTranscriptionJobsStore } from "@/stores/transcriptionJobsStore";

interface TranscriptionStatusProps {
  /** Where the jobs and their progress are. */
  onShowHistory: () => void;
}

/**
 * Footer status for live dictation first, with background History/file
 * transcriptions as a fallback; opens History when background jobs exist.
 */
const TranscriptionStatus: React.FC<TranscriptionStatusProps> = ({
  onShowHistory,
}) => {
  const { t } = useTranslation();
  const jobs = Object.values(useTranscriptionJobsStore((s) => s.jobs));
  const [recordingPhase, setRecordingPhase] = useState<RecordingPhase | null>(null);
  const [workingKind, setWorkingKind] = useState<"transcribing" | "polishing" | null>(null);

  useEffect(() => {
    const offState = events.recordingStateEvent.listen(({ payload }) => {
      setRecordingPhase(payload.phase);
      if (payload.phase === "started" || payload.phase === "locked") {
        setWorkingKind(null);
      }
      if (payload.phase === "cancelled") {
        setWorkingKind(null);
      }
    });
    const offPhase = events.streamPhaseEvent.listen(({ payload }) => {
      const event = payload as StreamPhaseEvent;
      if (event.phase === "working") {
        setWorkingKind(event.kind ?? "transcribing");
      }
    });
    const offDone = events.transcriptionCompleteEvent.listen(() => {
      setRecordingPhase(null);
      setWorkingKind(null);
    });
    return () => {
      offState.then((off) => off());
      offPhase.then((off) => off());
      offDone.then((off) => off());
    };
  }, []);

  const liveText = (() => {
    if (recordingPhase === "started" || recordingPhase === "locked") {
      return t("footer.recording");
    }
    if (recordingPhase === "stopped") {
      return workingKind === "polishing"
        ? t("overlay.processing")
        : t("overlay.transcribing");
    }
    return null;
  })();

  if (liveText) {
    return <span className="tabular-nums text-ink-3">{liveText}</span>;
  }

  if (jobs.length === 0) return null;

  // Jobs run one at a time; the rest wait behind it.
  const running = jobs.find((j) => j.state === "running");
  const text = !running
    ? t("settings.history.waiting")
    : running.percent > 0
      ? t("settings.history.transcribingPercent", { percent: running.percent })
      : t("settings.history.transcribing");

  return (
    <button
      type="button"
      onClick={onShowHistory}
      className="cursor-pointer tabular-nums text-ink-3 transition-colors hover:text-text focus:outline-none focus-visible:ring-1 focus-visible:ring-logo-primary"
    >
      {text}
    </button>
  );
};

export default TranscriptionStatus;
