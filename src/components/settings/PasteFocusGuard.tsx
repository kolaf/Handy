import React from "react";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

interface PasteFocusGuardProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const PasteFocusGuard: React.FC<PasteFocusGuardProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const enabled = getSetting("paste_focus_guard") ?? true;

    return (
      <ToggleSwitch
        checked={enabled}
        onChange={(value) => updateSetting("paste_focus_guard", value)}
        isUpdating={isUpdating("paste_focus_guard")}
        label={t("settings.advanced.pasteFocusGuard.label")}
        description={t("settings.advanced.pasteFocusGuard.description")}
        descriptionMode={descriptionMode}
        grouped={grouped}
      />
    );
  },
);
