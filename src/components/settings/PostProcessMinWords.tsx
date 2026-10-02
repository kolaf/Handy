import React from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../hooks/useSettings";
import { Input } from "../ui/Input";
import { SettingContainer } from "../ui/SettingContainer";

interface PostProcessMinWordsProps {
  descriptionMode?: "tooltip" | "inline";
  grouped?: boolean;
}

export const PostProcessMinWords: React.FC<PostProcessMinWordsProps> = ({
  descriptionMode = "tooltip",
  grouped = false,
}) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const minWords = getSetting("post_process_min_words") ?? 0;

  const handleChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    const value = parseInt(event.target.value, 10);
    if (!isNaN(value) && value >= 0 && value <= 50) {
      updateSetting("post_process_min_words", value);
    }
  };

  return (
    <SettingContainer
      title={t("settings.postProcessing.minWords.title")}
      description={t("settings.postProcessing.minWords.description")}
      descriptionMode={descriptionMode}
      grouped={grouped}
      layout="horizontal"
    >
      <div className="flex items-center space-x-2">
        <Input
          type="number"
          min="0"
          max="50"
          value={minWords}
          onChange={handleChange}
          disabled={isUpdating("post_process_min_words")}
          className="w-20"
        />
        <span className="text-sm text-text">
          {t("settings.postProcessing.minWords.unit")}
        </span>
      </div>
    </SettingContainer>
  );
};
