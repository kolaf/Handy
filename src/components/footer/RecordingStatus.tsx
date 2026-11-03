import React from "react";
import { useTranslation } from "react-i18next";
import {
  Mic,
  LoaderCircle,
  CheckCircle2,
  AlertCircle,
  CircleSlash,
} from "lucide-react";
import {
  events,
  type RecordingStateEvent,
  type TranscriptionCompleteEvent,
  type TranscriptionOutcome,
} from "@/bindings";

type StatusState =
  | { kind: "idle" }
  | { kind: "recording"; locked: boolean }
  | { kind: "transcribing" }
  | { kind: "done"; outcome: Exclude<TranscriptionOutcome, "failed"> }
  | { kind: "failed" };

const RESET_MS = 4000;

export const reduceRecordingPhase = (
  phase: RecordingStateEvent["phase"],
): StatusState => {
  switch (phase) {
    case "started":
      return { kind: "recording", locked: false };
    case "locked":
      return { kind: "recording", locked: true };
    case "stopped":
      return { kind: "transcribing" };
    case "cancelled":
      return { kind: "idle" };
  }
};

export const reduceTranscriptionOutcome = (
  outcome: TranscriptionCompleteEvent["outcome"],
): StatusState => {
  if (outcome === "failed") {
    return { kind: "failed" };
  }
  return { kind: "done", outcome };
};

const RecordingStatus: React.FC = () => {
  const { t } = useTranslation();
  const [status, setStatus] = React.useState<StatusState>({ kind: "idle" });

  React.useEffect(() => {
    let resetTimer: ReturnType<typeof setTimeout> | null = null;
    const armReset = () => {
      if (resetTimer) clearTimeout(resetTimer);
      resetTimer = setTimeout(() => setStatus({ kind: "idle" }), RESET_MS);
    };

    const offState = events.recordingStateEvent.listen(
      ({ payload }: { payload: RecordingStateEvent }) => {
        if (resetTimer) {
          clearTimeout(resetTimer);
          resetTimer = null;
        }
        setStatus(reduceRecordingPhase(payload.phase));
      },
    );

    const offDone = events.transcriptionCompleteEvent.listen(
      ({ payload }: { payload: TranscriptionCompleteEvent }) => {
        setStatus(reduceTranscriptionOutcome(payload.outcome));
        armReset();
      },
    );

    return () => {
      if (resetTimer) clearTimeout(resetTimer);
      offState.then((unlisten: () => void) => unlisten());
      offDone.then((unlisten: () => void) => unlisten());
    };
  }, []);

  const view = React.useMemo(() => {
    switch (status.kind) {
      case "idle":
        return null;
      case "recording":
        return {
          icon: Mic,
          className: "text-logo-primary",
          label: status.locked
            ? t("footer.recordingStatus.recordingLocked")
            : t("footer.recordingStatus.recording"),
        };
      case "transcribing":
        return {
          icon: LoaderCircle,
          className: "text-logo-primary animate-spin",
          label: t("footer.recordingStatus.transcribing"),
        };
      case "done":
        if (status.outcome === "typed") {
          return {
            icon: CheckCircle2,
            className: "text-green-500",
            label: t("footer.recordingStatus.typed"),
          };
        }
        if (status.outcome === "silent") {
          return {
            icon: CircleSlash,
            className: "text-ink-3",
            label: t("footer.recordingStatus.silent"),
          };
        }
        return {
          icon: CircleSlash,
          className: "text-ink-3",
          label: t("footer.recordingStatus.empty"),
        };
      case "failed":
        return {
          icon: AlertCircle,
          className: "text-red-500",
          label: t("footer.recordingStatus.failed"),
        };
    }
  }, [status, t]);

  if (!view) return null;

  const Icon = view.icon;
  return (
    <div
      className="inline-flex items-center gap-1.5 rounded-full border border-line px-2 py-1 text-ink-2"
      aria-live="polite"
    >
      <Icon size={14} className={view.className} />
      <span>{view.label}</span>
    </div>
  );
};

export default RecordingStatus;
