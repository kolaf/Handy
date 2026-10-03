import React from "react";
import ReactDOM from "react-dom/client";
import PromptPicker from "./PromptPicker";
import "@/i18n";
import "./PromptPicker.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <PromptPicker />
  </React.StrictMode>,
);
