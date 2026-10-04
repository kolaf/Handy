import React, { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import { commands, type MeetingSettings } from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { Button } from "../../ui/Button";
import { Dropdown } from "../../ui/Dropdown";
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

// Meeting minutes from audio files: transcribe (locally with the loaded speech model, or in the cloud), then write the
// minutes with the post-processing model. The result is saved, opened, and listed on the Activity page.
export const MeetingsSettings: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting } = useSettings();
  const saved: MeetingSettings = getSetting("meeting") ?? {};
  const [engine, setEngine] = useState(saved.engine ?? "local");
  const [language, setLanguage] = useState(saved.language ?? "en");
  const [endpoint, setEndpoint] = useState(saved.endpoint ?? "");
  const [apiVersion, setApiVersion] = useState(
    saved.api_version ?? "2025-03-01-preview",
  );
  const [model, setModel] = useState(
    saved.transcribe_model ?? "gpt-4o-transcribe",
  );
  const [outputDir, setOutputDir] = useState(saved.output_dir ?? "");
  const [apiKey, setApiKey] = useState("");
  const keySaved = Boolean(getSetting("post_process_api_keys")?.["meeting"]);
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

  const current = (): MeetingSettings => ({
    engine,
    language,
    endpoint,
    api_version: apiVersion,
    transcribe_model: model,
    output_dir: outputDir,
  });

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
    const saveResult = await commands.updateMeetingSettings(current());
    if (saveResult.status !== "ok") {
      setError(saveResult.error);
      return;
    }
    if (apiKey) {
      await commands.setMeetingApiKey(apiKey);
      setApiKey("");
    }
    const result = await commands.startMeeting(files, language, engine);
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
          <div className="flex gap-2 items-center">
            <Dropdown
              options={[
                { value: "local", label: t("settings.meetings.engineLocal") },
                { value: "cloud", label: t("settings.meetings.engineCloud") },
              ]}
              selectedValue={engine}
              onSelect={setEngine}
              disabled={running}
            />
            <Input
              type="text"
              value={language}
              onChange={(e) => setLanguage(e.target.value)}
              placeholder={t("settings.meetings.languagePlaceholder")}
              variant="compact"
              disabled={running}
            />
          </div>
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
      <SettingsGroup title={t("settings.meetings.settingsTitle")}>
        <div className="px-4 py-3 flex flex-col gap-2 text-sm">
          <Input
            type="text"
            value={outputDir}
            onChange={(e) => setOutputDir(e.target.value)}
            placeholder={t("settings.meetings.outputPlaceholder")}
            variant="compact"
          />
          <p className="text-xs text-mid-gray">
            {t("settings.meetings.cloudHelp")}
          </p>
          <Input
            type="text"
            value={endpoint}
            onChange={(e) => setEndpoint(e.target.value)}
            placeholder={t("settings.meetings.endpointPlaceholder")}
            variant="compact"
          />
          <Input
            type="text"
            value={model}
            onChange={(e) => setModel(e.target.value)}
            placeholder={t("settings.meetings.modelPlaceholder")}
            variant="compact"
          />
          <Input
            type="text"
            value={apiVersion}
            onChange={(e) => setApiVersion(e.target.value)}
            placeholder={t("settings.meetings.apiVersionPlaceholder")}
            variant="compact"
          />
          <Input
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            placeholder={
              keySaved
                ? t("settings.meetings.keySavedPlaceholder")
                : t("settings.meetings.keyPlaceholder")
            }
            variant="compact"
          />
          <p className="text-xs text-mid-gray">
            {t("settings.meetings.saveNote")}
          </p>
        </div>
      </SettingsGroup>
    </div>
  );
};
