import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { TauriApi } from "./api/TauriApi";
import "./app.css";
import { I18n } from "./i18n/I18n";
import { MainWindow, applyTextSize } from "./ui/screens/MainWindow";

const root = document.getElementById("root");
if (!root) throw new Error("no #root element");

const api = new TauriApi();

// Язык и размер текста читаются до первого кадра: окно не мигает чужим языком и размером. Язык выбирает Rust по языкам системы.
Promise.all([api.getLanguage(), api.getSettings()])
  .then(([language, settings]) => {
    applyTextSize(settings.textSize);
    createRoot(root).render(
      <StrictMode>
        <I18n language={language}>
          <MainWindow api={api} initialSettings={settings} />
        </I18n>
      </StrictMode>,
    );
  })
  .catch((reason: unknown) => {
    // Язык неизвестен, поэтому сообщение без переводов: оно появляется, только если Rust не отвечает совсем.
    root.textContent = `Kinige: ${String((reason as { code?: string } | null)?.code ?? reason)}`;
  });
