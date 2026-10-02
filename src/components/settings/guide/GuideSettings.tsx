import React from "react";
import { useTranslation } from "react-i18next";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { MarkdownContent } from "../../whats-new/MarkdownContent";
import forkGuide from "../../../content/fork-guide.md?raw";

export const GuideSettings: React.FC = () => {
  const { t } = useTranslation();

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup title={t("sidebar.guide")}>
        <div className="px-4 py-3">
          <MarkdownContent markdown={forkGuide} />
        </div>
      </SettingsGroup>
    </div>
  );
};
