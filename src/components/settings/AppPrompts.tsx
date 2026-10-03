import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../hooks/useSettings";
import { Button } from "../ui/Button";
import { Dropdown } from "../ui/Dropdown";
import { Input } from "../ui/Input";
import { SettingContainer } from "../ui/SettingContainer";
import { ToggleSwitch } from "../ui/ToggleSwitch";

interface AppPromptsProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const AppPrompts: React.FC<AppPromptsProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const [app, setApp] = useState("");
    const [title, setTitle] = useState("");
    const [promptId, setPromptId] = useState<string | null>(null);

    const enabled = getSetting("app_prompts_enabled") ?? false;
    const rules = getSetting("app_prompts") || [];
    const prompts = (getSetting("post_process_prompts") || []).filter(
      (p) => !p.id.startsWith("t_"),
    );
    const nameOf = (id: string) => prompts.find((p) => p.id === id)?.name ?? id;
    const canAdd = app.trim().length > 0 && promptId !== null;

    const handleAdd = () => {
      if (!canAdd || promptId === null) return;
      updateSetting("app_prompts", [
        ...rules,
        { app: app.trim(), title: title.trim(), prompt_id: promptId },
      ]);
      setApp("");
      setTitle("");
    };

    const handleRemove = (index: number) => {
      updateSetting(
        "app_prompts",
        rules.filter((_, i) => i !== index),
      );
    };

    return (
      <>
        <ToggleSwitch
          checked={enabled}
          onChange={(value) => updateSetting("app_prompts_enabled", value)}
          isUpdating={isUpdating("app_prompts_enabled")}
          label={t("settings.postProcessing.appPrompts.enabledLabel")}
          description={t(
            "settings.postProcessing.appPrompts.enabledDescription",
          )}
          descriptionMode={descriptionMode}
          grouped={grouped}
        />
        <SettingContainer
          title={t("settings.postProcessing.appPrompts.rulesTitle")}
          description={t(
            "settings.postProcessing.appPrompts.enabledDescription",
          )}
          descriptionMode={descriptionMode}
          grouped={grouped}
          layout="stacked"
        >
          <div className="flex flex-col gap-2 w-full">
            {rules.map((rule, index) => (
              <div
                key={`${rule.app}|${rule.title}|${index}`}
                className="flex items-center justify-between gap-3 text-sm"
              >
                <div className="min-w-0 break-words">
                  <span className="font-semibold">{rule.app}</span>
                  {rule.title ? (
                    <span className="text-mid-gray"> “{rule.title}”</span>
                  ) : null}
                  {" → "}
                  <span>{nameOf(rule.prompt_id)}</span>
                </div>
                <Button
                  onClick={() => handleRemove(index)}
                  disabled={isUpdating("app_prompts")}
                  variant="secondary"
                  size="sm"
                >
                  {t("settings.postProcessing.appPrompts.remove")}
                </Button>
              </div>
            ))}
            <Input
              type="text"
              value={app}
              onChange={(e) => setApp(e.target.value)}
              placeholder={t(
                "settings.postProcessing.appPrompts.appPlaceholder",
              )}
              variant="compact"
              disabled={isUpdating("app_prompts")}
            />
            <Input
              type="text"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder={t(
                "settings.postProcessing.appPrompts.titlePlaceholder",
              )}
              variant="compact"
              disabled={isUpdating("app_prompts")}
            />
            <Dropdown
              options={prompts.map((p) => ({ value: p.id, label: p.name }))}
              selectedValue={promptId}
              onSelect={setPromptId}
              placeholder={t(
                "settings.postProcessing.appPrompts.promptPlaceholder",
              )}
              disabled={isUpdating("app_prompts")}
            />
            <div>
              <Button
                onClick={handleAdd}
                disabled={!canAdd || isUpdating("app_prompts")}
                variant="primary"
                size="md"
              >
                {t("settings.postProcessing.appPrompts.add")}
              </Button>
            </div>
          </div>
        </SettingContainer>
      </>
    );
  },
);
