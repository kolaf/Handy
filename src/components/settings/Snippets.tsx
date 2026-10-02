import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { useSettings } from "../../hooks/useSettings";
import { Input } from "../ui/Input";
import { Textarea } from "../ui/Textarea";
import { Button } from "../ui/Button";
import { SettingContainer } from "../ui/SettingContainer";

interface SnippetsProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
}

const MAX_NAME = 50;
const MAX_TEXT = 5000;

const normalizeName = (name: string) =>
  name
    .replace(/[[\]\n\r]/g, "")
    .replace(/\s+/g, " ")
    .trim();

export const Snippets: React.FC<SnippetsProps> = React.memo(
  ({ descriptionMode = "tooltip", grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, updateSetting, isUpdating } = useSettings();
    const [name, setName] = useState("");
    const [text, setText] = useState("");
    const snippets = getSetting("snippets") || [];
    const normalizedName = normalizeName(name);
    const canAdd =
      normalizedName.length > 0 &&
      normalizedName.length <= MAX_NAME &&
      text.trim().length > 0 &&
      text.length <= MAX_TEXT;

    const handleAdd = () => {
      if (!canAdd) return;
      if (
        snippets.some(
          (s) => s.name.toLowerCase() === normalizedName.toLowerCase(),
        )
      ) {
        toast.error(
          t("settings.advanced.snippets.duplicate", { name: normalizedName }),
        );
        return;
      }
      updateSetting("snippets", [...snippets, { name: normalizedName, text }]);
      setName("");
      setText("");
    };

    const handleRemove = (nameToRemove: string) => {
      updateSetting(
        "snippets",
        snippets.filter((s) => s.name !== nameToRemove),
      );
    };

    return (
      <>
        <SettingContainer
          title={t("settings.advanced.snippets.title")}
          description={t("settings.advanced.snippets.description")}
          descriptionMode={descriptionMode}
          grouped={grouped}
          layout="stacked"
        >
          <div className="flex flex-col gap-2 w-full">
            <Input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t("settings.advanced.snippets.namePlaceholder")}
              variant="compact"
              disabled={isUpdating("snippets")}
            />
            <Textarea
              value={text}
              onChange={(e) => setText(e.target.value)}
              placeholder={t("settings.advanced.snippets.textPlaceholder")}
              variant="compact"
              disabled={isUpdating("snippets")}
            />
            <div>
              <Button
                onClick={handleAdd}
                disabled={!canAdd || isUpdating("snippets")}
                variant="primary"
                size="md"
              >
                {t("settings.advanced.snippets.add")}
              </Button>
            </div>
          </div>
        </SettingContainer>
        {snippets.length > 0 && (
          <div
            className={`px-4 p-2 ${grouped ? "" : "rounded-lg border border-mid-gray/20"} flex flex-col gap-2`}
          >
            {snippets.map((snippet) => (
              <div
                key={snippet.name}
                className="flex items-start justify-between gap-3 text-sm"
              >
                <div className="min-w-0">
                  <div className="font-semibold">{snippet.name}</div>
                  <div className="text-mid-gray whitespace-pre-wrap break-words line-clamp-3">
                    {snippet.text}
                  </div>
                </div>
                <Button
                  onClick={() => handleRemove(snippet.name)}
                  disabled={isUpdating("snippets")}
                  variant="secondary"
                  size="sm"
                  aria-label={t("settings.advanced.snippets.remove", {
                    name: snippet.name,
                  })}
                >
                  {t("settings.advanced.snippets.removeShort")}
                </Button>
              </div>
            ))}
          </div>
        )}
      </>
    );
  },
);
