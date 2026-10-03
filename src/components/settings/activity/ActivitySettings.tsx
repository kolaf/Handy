import React, { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { commands, type ActivityEntry } from "@/bindings";
import { Button } from "../../ui/Button";

const formatTime = (timestamp: number, locale: string): string =>
  new Date(timestamp).toLocaleString(locale, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });

// The log of on-screen notices ("toasts") with the details behind them: what was compared, what the model said, what was
// added and what was dropped and why.
export const ActivitySettings: React.FC = () => {
  const { t, i18n } = useTranslation();
  const [entries, setEntries] = useState<ActivityEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [open, setOpen] = useState<number | null>(null);

  const load = useCallback(async () => {
    try {
      setEntries(await commands.getActivity());
    } catch (error) {
      console.error("Failed to load the activity log:", error);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
    const unlisten = listen("activity-added", () => load());
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [load]);

  const clear = async () => {
    await commands.clearActivity();
    setOpen(null);
    await load();
  };

  const kindLabel = (kind: string) =>
    t(`settings.activity.kinds.${kind}`, { defaultValue: kind });

  let content: React.ReactNode;
  if (loading) {
    content = (
      <div className="px-4 py-3 text-center text-text/60">
        {t("settings.activity.loading")}
      </div>
    );
  } else if (entries.length === 0) {
    content = (
      <div className="px-4 py-3 text-center text-text/60">
        {t("settings.activity.empty")}
      </div>
    );
  } else {
    content = (
      <div className="divide-y divide-mid-gray/20">
        {entries.map((entry) => (
          <div key={`${entry.timestamp}-${entry.kind}`} className="px-4 py-3">
            <button
              className="w-full text-left cursor-pointer"
              onClick={() =>
                setOpen(open === entry.timestamp ? null : entry.timestamp)
              }
            >
              <div className="flex items-baseline justify-between gap-3">
                <span className="text-sm font-semibold">
                  {kindLabel(entry.kind)}
                </span>
                <span className="text-xs text-mid-gray shrink-0">
                  {formatTime(entry.timestamp, i18n.language)}
                </span>
              </div>
              {entry.title ? (
                <div className="text-sm text-text/80 break-words">
                  {entry.title}
                </div>
              ) : null}
              {entry.details && open !== entry.timestamp ? (
                <div className="text-xs text-mid-gray">
                  {t("settings.activity.showDetails")}
                </div>
              ) : null}
            </button>
            {entry.details && open === entry.timestamp ? (
              <pre className="mt-2 text-xs whitespace-pre-wrap break-words text-text/70 select-text">
                {entry.details}
              </pre>
            ) : null}
          </div>
        ))}
      </div>
    );
  }

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <div className="space-y-2">
        <div className="px-4 flex items-center justify-between">
          <h2 className="text-xs font-medium text-mid-gray uppercase tracking-wide">
            {t("settings.activity.title")}
          </h2>
          <Button
            onClick={clear}
            disabled={entries.length === 0}
            variant="secondary"
            size="sm"
          >
            <Trash2 size={14} className="inline mr-1" />
            {t("settings.activity.clear")}
          </Button>
        </div>
        <p className="px-4 text-xs text-mid-gray">
          {t("settings.activity.description")}
        </p>
        <div className="bg-background border border-mid-gray/20 rounded-lg overflow-visible">
          {content}
        </div>
      </div>
    </div>
  );
};
