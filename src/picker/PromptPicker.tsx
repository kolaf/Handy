import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface PickerItem {
  number: number;
  id: string;
  name: string;
}

interface OpenPayload {
  mode: string;
  items: PickerItem[];
  selected: string | null;
}

// The list of post-processing prompts, shown in its own non-focusable window so the app being dictated into keeps
// its selection. The number keys are handled by the backend (temporary global shortcuts); clicks come here.
const PromptPicker: React.FC = () => {
  const { t } = useTranslation();
  const [items, setItems] = useState<PickerItem[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [mode, setMode] = useState("prompts");

  useEffect(() => {
    // The window is created the first time the picker opens, so its page may load after the "picker-open" event
    // was sent. Ask for the current list as well.
    invoke<OpenPayload | null>("picker_state").then((state) => {
      if (state) {
        setItems(state.items);
        setSelected(state.selected);
        setMode(state.mode);
      }
    });
    const unlistenOpen = listen<OpenPayload>("picker-open", (event) => {
      setItems(event.payload.items);
      setSelected(event.payload.selected);
      setMode(event.payload.mode);
    });
    const unlistenClose = listen("picker-close", () => setItems([]));
    return () => {
      unlistenOpen.then((f) => f());
      unlistenClose.then((f) => f());
    };
  }, []);

  return (
    <div className="picker-card">
      <div className="picker-header">
        <span>
          {mode === "models" ? t("picker.modelsTitle") : t("picker.title")}
        </span>
        <button
          className="picker-close"
          aria-label={t("picker.close")}
          onClick={() => invoke("picker_close")}
        >
          {t("picker.escape")}
        </button>
      </div>
      {items.map((item) => (
        <button
          key={item.id}
          className={`picker-row${item.id === selected ? " picker-row-active" : ""}`}
          onClick={() => invoke("picker_select", { number: item.number })}
        >
          <span className="picker-name">{item.name}</span>
          <span className="picker-number">{item.number}</span>
        </button>
      ))}
    </div>
  );
};

export default PromptPicker;
