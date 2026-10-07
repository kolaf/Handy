import React, { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import { commands, type MeetingSettings, type ModelInfo } from "@/bindings";
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

// Meeting minutes from audio files: transcribed with the speech model Handy has loaded, then written up by the
// post-processing model. The result is saved, opened, and listed on the Activity page.
export const MeetingsSettings: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting } = useSettings();
  const saved: MeetingSettings = getSetting("meeting") ?? {};
  const [language, setLanguage] = useState(saved.language ?? "en");
  const [outputDir, setOutputDir] = useState(saved.output_dir ?? "");
  const [modelId, setModelId] = useState(saved.model_id ?? "");
  const [recordingsDir, setRecordingsDir] = useState(
    saved.recordings_dir ?? "",
  );
  const [groupMinutes, setGroupMinutes] = useState(
    String(saved.group_minutes ?? 5),
  );
  const [skipSilence, setSkipSilence] = useState(saved.skip_silence ?? true);
  const [speakers, setSpeakers] = useState(saved.speakers ?? false);
  const [nameSpeakers, setNameSpeakers] = useState(saved.name_speakers ?? true);
  const [diarizeModel, setDiarizeModel] = useState(
    saved.diarize_model ?? "gpt-4o-transcribe-diarize",
  );
  const [sbUrl, setSbUrl] = useState(saved.silverbullet_url ?? "");
  const [sbToken, setSbToken] = useState(saved.silverbullet_token ?? "");
  const [sbFolder, setSbFolder] = useState(
    saved.silverbullet_folder ?? "Meeting Notes",
  );
  const [sbTag, setSbTag] = useState(saved.silverbullet_tag ?? "fromMeeting");
  const [project, setProject] = useState("");
  const [projects, setProjects] = useState<string[]>([]);
  const [projectsError, setProjectsError] = useState<string | null>(null);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [dragging, setDragging] = useState(false);
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
    commands.getAvailableModels().then((result) => {
      if (result.status === "ok") setModels(result.data);
    });
    // Files (or folders) dropped on the window are added to the list.
    const unlistenDrop = getCurrentWebview().onDragDropEvent((event) => {
      const payload = event.payload;
      if (payload.type === "enter" || payload.type === "over")
        setDragging(true);
      else if (payload.type === "leave") setDragging(false);
      else if (payload.type === "drop") {
        setDragging(false);
        setFiles((previous) => [
          ...previous,
          ...payload.paths.filter((p) => !previous.includes(p)),
        ]);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
      unlistenDrop.then((fn) => fn());
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

  const chooseFolder = async () => {
    const picked = await open({ directory: true });
    if (typeof picked === "string") setFiles([picked]);
  };

  const chooseRecordingsFolder = async () => {
    const picked = await open({ directory: true });
    if (typeof picked === "string") setRecordingsDir(picked);
  };

  const settingsToSave = (): MeetingSettings => ({
    language,
    output_dir: outputDir,
    model_id: modelId,
    recordings_dir: recordingsDir,
    group_minutes: Math.max(1, Number.parseInt(groupMinutes, 10) || 5),
    skip_silence: skipSilence,
    speakers,
    name_speakers: nameSpeakers,
    diarize_model: diarizeModel.trim() || "gpt-4o-transcribe-diarize",
    silverbullet_url: sbUrl.trim(),
    silverbullet_token: sbToken.trim(),
    silverbullet_folder: sbFolder.trim() || "Meeting Notes",
    silverbullet_tag: sbTag.trim() || "fromMeeting",
  });

  // The settings are saved first: the projects are read with the saved address and token.
  const refreshProjects = async () => {
    setProjectsError(null);
    const saveResult = await commands.updateMeetingSettings(settingsToSave());
    if (saveResult.status !== "ok") {
      setProjectsError(saveResult.error);
      return;
    }
    const result = await commands.listSilverbulletProjects();
    if (result.status === "ok") {
      setProjects(result.data);
      if (!result.data.includes(project)) setProject("");
    } else {
      setProjectsError(result.error);
    }
  };

  // Fills the list with the latest recording from the recorder's folder (the files that belong together).
  const useLatest = async () => {
    setError(null);
    const saveResult = await commands.updateMeetingSettings(settingsToSave());
    if (saveResult.status !== "ok") {
      setError(saveResult.error);
      return;
    }
    const result = await commands.findLatestRecordings(false);
    if (result.status === "ok") setFiles(result.data);
    else setError(result.error);
  };

  const start = async () => {
    setError(null);
    const saveResult = await commands.updateMeetingSettings(settingsToSave());
    if (saveResult.status !== "ok") {
      setError(saveResult.error);
      return;
    }
    const result = await commands.startMeeting(
      files,
      language,
      project || null,
    );
    if (result.status !== "ok") setError(result.error);
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup title={t("settings.meetings.title")}>
        <div className="px-4 py-3 flex flex-col gap-3 text-sm">
          <p className="text-mid-gray">{t("settings.meetings.description")}</p>
          <div className="flex flex-col gap-2">
            <div
              className={`rounded-lg border border-dashed px-3 py-4 text-center text-mid-gray ${
                dragging
                  ? "border-logo-primary bg-logo-primary/10"
                  : "border-mid-gray/40"
              }`}
            >
              {t("settings.meetings.dropHere")}
            </div>
            <div className="flex gap-2">
              <Button
                onClick={choose}
                variant="secondary"
                size="md"
                disabled={running}
              >
                {t("settings.meetings.chooseFiles")}
              </Button>
              <Button
                onClick={chooseFolder}
                variant="secondary"
                size="md"
                disabled={running}
              >
                {t("settings.meetings.chooseFolder")}
              </Button>
              <Button
                onClick={useLatest}
                variant="secondary"
                size="md"
                disabled={running}
              >
                {t("settings.meetings.useLatest")}
              </Button>
            </div>
            {files.map((file) => (
              <div key={file} className="break-all text-xs text-text/70">
                {file}
              </div>
            ))}
          </div>
          <Dropdown
            options={[
              { value: "", label: t("settings.meetings.modelSame") },
              ...models
                .filter((m) => m.is_downloaded)
                .map((m) => ({ value: m.id, label: m.name })),
            ]}
            selectedValue={modelId}
            onSelect={setModelId}
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
          <div className="flex gap-2 items-center">
            <Input
              type="text"
              value={recordingsDir}
              onChange={(e) => setRecordingsDir(e.target.value)}
              placeholder={t("settings.meetings.recordingsPlaceholder")}
              variant="compact"
              disabled={running}
            />
            <Button
              onClick={chooseRecordingsFolder}
              variant="secondary"
              size="sm"
              disabled={running}
            >
              {t("settings.meetings.browse")}
            </Button>
          </div>
          <Input
            type="text"
            value={groupMinutes}
            onChange={(e) => setGroupMinutes(e.target.value)}
            placeholder={t("settings.meetings.groupPlaceholder")}
            variant="compact"
            disabled={running}
          />
          <label className="flex items-start gap-2">
            <input
              type="checkbox"
              checked={skipSilence}
              onChange={(e) => setSkipSilence(e.target.checked)}
              disabled={running}
              className="mt-1"
            />
            <span>
              {t("settings.meetings.skipSilence")}
              <span className="block text-xs text-mid-gray">
                {t("settings.meetings.skipSilenceDescription")}
              </span>
            </span>
          </label>
          <label className="flex items-start gap-2">
            <input
              type="checkbox"
              checked={speakers}
              onChange={(e) => setSpeakers(e.target.checked)}
              disabled={running}
              className="mt-1"
            />
            <span>
              {t("settings.meetings.speakers")}
              <span className="block text-xs text-mid-gray">
                {t("settings.meetings.speakersDescription")}
              </span>
            </span>
          </label>
          {speakers ? (
            <label className="flex items-start gap-2">
              <input
                type="checkbox"
                checked={nameSpeakers}
                onChange={(e) => setNameSpeakers(e.target.checked)}
                disabled={running}
                className="mt-1"
              />
              <span>
                {t("settings.meetings.nameSpeakers")}
                <span className="block text-xs text-mid-gray">
                  {t("settings.meetings.nameSpeakersDescription")}
                </span>
              </span>
            </label>
          ) : null}
          {speakers ? (
            <Input
              type="text"
              value={diarizeModel}
              onChange={(e) => setDiarizeModel(e.target.value)}
              placeholder={t("settings.meetings.diarizeModelPlaceholder")}
              variant="compact"
              disabled={running}
            />
          ) : null}
          <div className="flex flex-col gap-2 rounded-lg border border-mid-gray/20 p-3">
            <div className="font-semibold">
              {t("settings.meetings.silverbullet.title")}
            </div>
            <p className="text-xs text-mid-gray">
              {t("settings.meetings.silverbullet.description")}
            </p>
            <Input
              type="text"
              value={sbUrl}
              onChange={(e) => setSbUrl(e.target.value)}
              placeholder={t("settings.meetings.silverbullet.urlPlaceholder")}
              variant="compact"
              disabled={running}
            />
            <Input
              type="password"
              value={sbToken}
              onChange={(e) => setSbToken(e.target.value)}
              placeholder={t("settings.meetings.silverbullet.tokenPlaceholder")}
              variant="compact"
              disabled={running}
            />
            <Input
              type="text"
              value={sbFolder}
              onChange={(e) => setSbFolder(e.target.value)}
              placeholder={t(
                "settings.meetings.silverbullet.folderPlaceholder",
              )}
              variant="compact"
              disabled={running}
            />
            <Input
              type="text"
              value={sbTag}
              onChange={(e) => setSbTag(e.target.value)}
              placeholder={t("settings.meetings.silverbullet.tagPlaceholder")}
              variant="compact"
              disabled={running}
            />
            <div className="flex gap-2 items-center">
              <Dropdown
                options={[
                  {
                    value: "",
                    label: t("settings.meetings.silverbullet.noProject"),
                  },
                  ...projects.map((name) => ({ value: name, label: name })),
                ]}
                selectedValue={project}
                onSelect={setProject}
                disabled={running}
              />
              <Button
                onClick={refreshProjects}
                variant="secondary"
                size="sm"
                disabled={running || sbUrl.trim() === ""}
              >
                {t("settings.meetings.silverbullet.refresh")}
              </Button>
            </div>
            {projectsError ? (
              <div className="text-xs text-red-500 break-words">
                {projectsError}
              </div>
            ) : null}
          </div>
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
