import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands, type ModelInfo } from "@/bindings";
import { useSettings } from "../../hooks/useSettings";
import { Button } from "../ui/Button";
import { Dropdown } from "../ui/Dropdown";
import { Input } from "../ui/Input";
import { SettingContainer } from "../ui/SettingContainer";
import { ToggleSwitch } from "../ui/ToggleSwitch";

interface LanguageModelsProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

// Optional link between the dictation language and the speech model: choosing a language (swap shortcut, --set-language,
// the language setting) also switches to the model named for it, for example English → Parakeet, Norwegian → NB-Whisper.
export const LanguageModels: React.FC<LanguageModelsProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const [models, setModels] = useState<ModelInfo[]>([]);
    const [language, setLanguage] = useState("");
    const [modelId, setModelId] = useState<string | null>(null);

    useEffect(() => {
      commands.getAvailableModels().then((result) => {
        if (result.status === "ok") setModels(result.data);
      });
    }, []);

    const enabled = getSetting("language_models_enabled") ?? false;
    const rules = getSetting("language_models") || [];
    const nameOf = (id: string) => models.find((m) => m.id === id)?.name ?? id;
    const downloaded = models.filter((m) => m.is_downloaded);
    const canAdd = language.trim().length > 0 && modelId !== null;

    const handleAdd = () => {
      if (!canAdd || modelId === null) return;
      const code = language.trim();
      const others = rules.filter(
        (r) => r.language.toLowerCase() !== code.toLowerCase(),
      );
      updateSetting("language_models", [
        ...others,
        { language: code, model_id: modelId },
      ]);
      setLanguage("");
    };

    return (
      <>
        <ToggleSwitch
          checked={enabled}
          onChange={(value) => updateSetting("language_models_enabled", value)}
          isUpdating={isUpdating("language_models_enabled")}
          label={t("settings.general.languageModels.enabledLabel")}
          description={t("settings.general.languageModels.enabledDescription")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        />
        <SettingContainer
          title={t("settings.general.languageModels.rulesTitle")}
          description={t("settings.general.languageModels.enabledDescription")}
          descriptionMode={descriptionMode}
          grouped={grouped}
          layout="stacked"
        >
          <div className="flex flex-col gap-2 w-full">
            {rules.map((rule) => (
              <div
                key={rule.language}
                className="flex items-center justify-between gap-3 text-sm"
              >
                <div className="min-w-0 break-words">
                  <span className="font-semibold">{rule.language}</span>
                  {" → "}
                  <span>{nameOf(rule.model_id)}</span>
                </div>
                <Button
                  onClick={() =>
                    updateSetting(
                      "language_models",
                      rules.filter((r) => r.language !== rule.language),
                    )
                  }
                  disabled={isUpdating("language_models")}
                  variant="secondary"
                  size="sm"
                >
                  {t("settings.general.languageModels.remove")}
                </Button>
              </div>
            ))}
            <Input
              type="text"
              value={language}
              onChange={(e) => setLanguage(e.target.value)}
              placeholder={t(
                "settings.general.languageModels.languagePlaceholder",
              )}
              variant="compact"
              disabled={isUpdating("language_models")}
            />
            <Dropdown
              options={downloaded.map((m) => ({ value: m.id, label: m.name }))}
              selectedValue={modelId}
              onSelect={setModelId}
              placeholder={t(
                "settings.general.languageModels.modelPlaceholder",
              )}
              disabled={isUpdating("language_models")}
            />
            <div>
              <Button
                onClick={handleAdd}
                disabled={!canAdd || isUpdating("language_models")}
                variant="primary"
                size="md"
              >
                {t("settings.general.languageModels.add")}
              </Button>
            </div>
          </div>
        </SettingContainer>
      </>
    );
  },
);
