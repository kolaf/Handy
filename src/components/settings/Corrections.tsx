import React from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../hooks/useSettings";
import { Button } from "../ui/Button";
import { SettingContainer } from "../ui/SettingContainer";

interface CorrectionsProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const Corrections: React.FC<CorrectionsProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const corrections = getSetting("corrections") || [];

    const handleToggleHint = (wrong: string, right: string) => {
      updateSetting(
        "corrections",
        corrections.map((c) =>
          c.wrong === wrong && c.right === right ? { ...c, hint: !c.hint } : c,
        ),
      );
    };

    const handleRemove = (wrong: string, right: string) => {
      updateSetting(
        "corrections",
        corrections.filter((c) => !(c.wrong === wrong && c.right === right)),
      );
    };

    return (
      <>
        <SettingContainer
          title={t("settings.advanced.corrections.title")}
          description={t("settings.advanced.corrections.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
          layout="stacked"
        >
          {corrections.length === 0 ? (
            <div className="text-sm text-mid-gray">—</div>
          ) : (
            <div className="flex flex-col gap-2 w-full">
              {corrections.map((c) => (
                <div
                  key={`${c.wrong}→${c.right}`}
                  className="flex items-center justify-between gap-3 text-sm"
                >
                  <div className="min-w-0 break-words">
                    <span className="text-mid-gray">{c.wrong}</span>
                    {" → "}
                    <span className="font-semibold">{c.right}</span>
                  </div>
                  <div className="flex gap-2 shrink-0">
                    <Button
                      onClick={() => handleToggleHint(c.wrong, c.right)}
                      disabled={isUpdating("corrections")}
                      variant="secondary"
                      size="sm"
                      title={t("settings.advanced.corrections.modeHelp")}
                    >
                      {c.hint
                        ? t("settings.advanced.corrections.modeHint")
                        : t("settings.advanced.corrections.modeAlways")}
                    </Button>
                    <Button
                      onClick={() => handleRemove(c.wrong, c.right)}
                      disabled={isUpdating("corrections")}
                      variant="secondary"
                      size="sm"
                      aria-label={t("settings.advanced.corrections.remove", {
                        wrong: c.wrong,
                      })}
                    >
                      {t("settings.advanced.corrections.removeShort")}
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </SettingContainer>
      </>
    );
  },
);
