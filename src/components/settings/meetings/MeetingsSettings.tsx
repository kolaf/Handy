import React, { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import { commands, type MeetingSettings } from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { Button } from "../../ui/Button";
import { Input } from "../../ui/Input";
import { SettingsGroup } from "../../ui/SettingsGroup";

interface Progress {
  stage: string;
  done: number;
  total: number;
  message: string;
  path: string | null;
}

const AUDIO_EXTENSIONS = ["mp3", "m4a", "aac", "wav", "flac", "ogg"];

// Meeting minutes from audio files: transcribed with the speech model Handy has loaded, then written up by the
// post-processing model. The result is saved, opened, and listed on the Activity page.
export const MeetingsSettings: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting } = useSettings();
  const saved: MeetingSettings = getSetting("meeting") ?? {};
  const [language, setLanguage] = useState(saved.language ?? "en");
  const [outputDir, setOutputDir] = useState(saved.output_dir ?? "");
  const [files, setFiles] = useState<string[]>([]);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const running =
    progress !== null &&
    !["done", "failed", "cancelled"].includes(progress.stage);

  useEffect(() => {
    const unlisten = listen<Progress>("meeting-progress", (event) =>
      setProgress(event.payload),
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const choose = async () => {
    const picked = await open({
      multiple: true,
      filters: [
        {
          name: t("settings.meetings.audioFiles"),
          extensions: AUDIO_EXTENSIONS,
        },
      ],
    });
    if (Array.isArray(picked)) setFiles(picked);
    else if (typeof picked === "string") setFiles([picked]);
  };

  const start = async () => {
    setError(null);
    const saveResult = await commands.updateMeetingSettings({
      language,
      output_dir: outputDir,
    });
    if (saveResult.status !== "ok") {
      setError(saveResult.error);
      return;
    }
    const result = await commands.startMeeting(files, language);
    if (result.status !== "ok") setError(result.error);
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup title={t("settings.meetings.title")}>
        <div className="px-4 py-3 flex flex-col gap-3 text-sm">
          <p className="text-mid-gray">{t("settings.meetings.description")}</p>
          <div className="flex flex-col gap-2">
            <Button
              onClick={choose}
              variant="secondary"
              size="md"
              disabled={running}
            >
              {t("settings.meetings.chooseFiles")}
            </Button>
            {files.map((file) => (
              <div key={file} className="break-all text-xs text-text/70">
                {file}
              </div>
            ))}
          </div>
          <Input
            type="text"
            value={language}
            onChange={(e) => setLanguage(e.target.value)}
            placeholder={t("settings.meetings.languagePlaceholder")}
            variant="compact"
            disabled={running}
          />
          <Input
            type="text"
            value={outputDir}
            onChange={(e) => setOutputDir(e.target.value)}
            placeholder={t("settings.meetings.outputPlaceholder")}
            variant="compact"
            disabled={running}
          />
          <div className="flex gap-2">
            <Button
              onClick={start}
              variant="primary"
              size="md"
              disabled={running || files.length === 0}
            >
              {t("settings.meetings.start")}
            </Button>
            {running ? (
              <Button
                onClick={() => commands.cancelMeeting()}
                variant="secondary"
                size="md"
              >
                {t("settings.meetings.cancel")}
              </Button>
            ) : null}
          </div>
          {progress ? (
            <div className="space-y-1">
              <div>
                {t(`settings.meetings.stages.${progress.stage}`, {
                  defaultValue: progress.stage,
                })}
                {progress.total > 0
                  ? ` (${progress.done}/${progress.total})`
                  : ""}
              </div>
              <div className="text-xs text-text/70 break-words">
                {progress.message}
              </div>
              {progress.path ? (
                <Button
                  onClick={() =>
                    commands.openMeetingFile(progress.path as string)
                  }
                  variant="secondary"
                  size="sm"
                >
                  {t("settings.meetings.openMinutes")}
                </Button>
              ) : null}
            </div>
          ) : null}
          {error ? <div className="text-red-500">{error}</div> : null}
        </div>
      </SettingsGroup>
    </div>
  );
};
