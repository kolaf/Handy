import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../hooks/useSettings";
import { Button } from "../ui/Button";
import { Dropdown } from "../ui/Dropdown";
import { Input } from "../ui/Input";
import { SettingContainer } from "../ui/SettingContainer";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useOsType } from "../../hooks/useOsType";
import type { AppPasteMethod, PasteMethod } from "@/bindings";

interface AppPasteMethodsProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

export const AppPasteMethods: React.FC<AppPasteMethodsProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const osType = useOsType();
    const [app, setApp] = useState("");
    const [title, setTitle] = useState("");
    const [pasteMethod, setPasteMethod] = useState<PasteMethod | null>(null);

    const enabled = getSetting("app_paste_methods_enabled") ?? false;
    const rules = (getSetting("app_paste_methods") || []) as AppPasteMethod[];

    const mod = osType === "macos" ? "Cmd" : "Ctrl";
    const pasteMethodOptions = [
      {
        value: "ctrl_v",
        label: t("settings.advanced.pasteMethod.options.clipboard", {
          modifier: mod,
        }),
      },
      {
        value: "ctrl_shift_v",
        label: t("settings.advanced.pasteMethod.options.clipboardCtrlShiftV"),
        disabled: !(osType === "windows" || osType === "linux"),
      },
      {
        value: "shift_insert",
        label: t("settings.advanced.pasteMethod.options.clipboardShiftInsert"),
        disabled: !(osType === "windows" || osType === "linux"),
      },
      {
        value: "direct",
        label: t("settings.advanced.pasteMethod.options.direct"),
        disabled: osType === "macos",
      },
      {
        value: "none",
        label: t("settings.advanced.pasteMethod.options.none"),
      },
      {
        value: "external_script",
        label: t("settings.advanced.pasteMethod.options.externalScript"),
        disabled: osType !== "linux",
      },
    ].filter((option) => !option.disabled || option.value === pasteMethod);

    const canAdd = app.trim().length > 0 && pasteMethod !== null;

    const handleAdd = () => {
      if (!canAdd || pasteMethod === null) return;
      updateSetting("app_paste_methods", [
        ...rules,
        { app: app.trim(), title: title.trim(), paste_method: pasteMethod },
      ]);
      setApp("");
      setTitle("");
      setPasteMethod(null);
    };

    const handleRemove = (index: number) => {
      updateSetting(
        "app_paste_methods",
        rules.filter((_, i) => i !== index),
      );
    };

    return (
      <>
        <ToggleSwitch
          checked={enabled}
          onChange={(value) => updateSetting("app_paste_methods_enabled", value)}
          isUpdating={isUpdating("app_paste_methods_enabled")}
          label={t("settings.advanced.appPasteMethods.enabledLabel")}
          description={t("settings.advanced.appPasteMethods.enabledDescription")}
          descriptionMode={descriptionMode}
          grouped={grouped}
        />
        <SettingContainer
          title={t("settings.advanced.appPasteMethods.rulesTitle")}
          description={t("settings.advanced.appPasteMethods.rulesDescription")}
          descriptionMode={descriptionMode}
          grouped={grouped}
          layout="stacked"
        >
          <div className="flex flex-col gap-2 w-full">
            {rules.map((rule, index) => (
              <div
                key={`${rule.app}|${rule.title}|${rule.paste_method}|${index}`}
                className="flex items-center justify-between gap-3 text-sm"
              >
                <div className="min-w-0 break-words">
                  <span className="font-semibold">{rule.app}</span>
                  {rule.title ? (
                    <span className="text-mid-gray"> “{rule.title}”</span>
                  ) : null}
                  {" → "}
                  <span>
                    {pasteMethodOptions.find((option) => option.value === rule.paste_method)
                      ?.label ?? rule.paste_method}
                  </span>
                </div>
                <Button
                  onClick={() => handleRemove(index)}
                  disabled={isUpdating("app_paste_methods")}
                  variant="secondary"
                  size="sm"
                >
                  {t("settings.advanced.appPasteMethods.remove")}
                </Button>
              </div>
            ))}
            <Input
              type="text"
              value={app}
              onChange={(e) => setApp(e.target.value)}
              placeholder={t("settings.advanced.appPasteMethods.appPlaceholder")}
              variant="compact"
              disabled={isUpdating("app_paste_methods")}
            />
            <Input
              type="text"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder={t("settings.advanced.appPasteMethods.titlePlaceholder")}
              variant="compact"
              disabled={isUpdating("app_paste_methods")}
            />
            <Dropdown
              options={pasteMethodOptions}
              selectedValue={pasteMethod}
              onSelect={(value) => setPasteMethod(value as PasteMethod)}
              placeholder={t("settings.advanced.appPasteMethods.pasteMethodPlaceholder")}
              disabled={isUpdating("app_paste_methods")}
            />
            <div>
              <Button
                onClick={handleAdd}
                disabled={!canAdd || isUpdating("app_paste_methods")}
                variant="primary"
                size="md"
              >
                {t("settings.advanced.appPasteMethods.add")}
              </Button>
            </div>
          </div>
        </SettingContainer>
      </>
    );
  },
);
