import React from "react";
import { useTranslation } from "react-i18next";
import { Dropdown } from "../ui/Dropdown";
import { SettingContainer } from "../ui/SettingContainer";
import { useSettings } from "../../hooks/useSettings";
import { useOsType } from "../../hooks/useOsType";
import type { AppSettings, AutoSubmitKey } from "@/bindings";

interface AutoSubmitProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
  titleKey?: string;
  descriptionKey?: string;
  enabledSettingKey?: keyof AppSettings;
  keySettingKey?: keyof AppSettings;
}

type AutoSubmitOptionValue = AutoSubmitKey | "off";

export const AutoSubmit: React.FC<AutoSubmitProps> = React.memo(
  ({
    descriptionMode = "tooltip",
    grouped = false,
    titleKey = "settings.advanced.autoSubmit.title",
    descriptionKey = "settings.advanced.autoSubmit.description",
    enabledSettingKey = "auto_submit",
    keySettingKey = "auto_submit_key",
  }) => {
    const { t } = useTranslation();
    const osType = useOsType();
    const { getSetting, updateSetting, isUpdating } = useSettings();

    const enabled = (getSetting(enabledSettingKey) ?? false) as boolean;
    const selectedKey = (getSetting(keySettingKey) ||
      "enter") as AutoSubmitKey;
    const selectedValue: AutoSubmitOptionValue = enabled ? selectedKey : "off";
    const submitWithMetaLabel =
      osType === "macos"
        ? t("settings.advanced.autoSubmit.options.cmdEnter")
        : t("settings.advanced.autoSubmit.options.superEnter");

    const autoSubmitOptions = [
      {
        value: "off",
        label: t("settings.advanced.autoSubmit.options.off"),
      },
      {
        value: "enter",
        label: t("settings.advanced.autoSubmit.options.enter"),
      },
      {
        value: "ctrl_enter",
        label: t("settings.advanced.autoSubmit.options.ctrlEnter"),
      },
      {
        value: "cmd_enter",
        label: submitWithMetaLabel,
      },
    ];

    const handleAutoSubmitSelect = async (value: string) => {
      const selected = value as AutoSubmitOptionValue;

      if (selected === "off") {
        await updateSetting(enabledSettingKey, false as AppSettings[keyof AppSettings]);
        return;
      }

      await updateSetting(
        keySettingKey,
        selected as AppSettings[keyof AppSettings],
      );
      if (!enabled) {
        await updateSetting(enabledSettingKey, true as AppSettings[keyof AppSettings]);
      }
    };

    return (
      <SettingContainer
        title={t(titleKey)}
        description={t(descriptionKey)}
        descriptionMode={descriptionMode}
        grouped={grouped}
      >
        <Dropdown
          options={autoSubmitOptions}
          selectedValue={selectedValue}
          onSelect={handleAutoSubmitSelect}
          disabled={isUpdating(String(enabledSettingKey)) || isUpdating(String(keySettingKey))}
        />
      </SettingContainer>
    );
  },
);
