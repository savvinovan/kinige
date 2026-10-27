import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { TauriApi } from "./api/TauriApi";
import "./app.css";
import { CaptureWindow } from "./capture/CaptureWindow";
import { I18n } from "./i18n/I18n";
import { AppRoot, RECOVERY_SETTINGS } from "./ui/screens/AppRoot";
import { applyTextSize } from "./ui/screens/textSize";

const root = document.getElementById("root");
if (!root) throw new Error("no #root element");

const api = new TauriApi();

/** Одна страница на два окна: окно захвата открывает её с `#capture` (`tauri.conf.json`), главное — без хэша. */
const isCaptureWindow = window.location.hash === "#capture";
// Окно захвата прозрачное (форму и тень панели рисует CSS): странице нужна метка, чтобы убрать фон (ui/capture/capture.css).
if (isCaptureWindow) document.documentElement.dataset.window = "capture";

// Язык и размер текста читаются до первого кадра: окно не мигает чужим языком и размером. Язык выбирает Rust по языкам системы.
const start = isCaptureWindow
  ? api.captureState().then((state) => {
      applyTextSize(state.textSize);
      createRoot(root).render(
        <StrictMode>
          <I18n language={state.language}>
            <CaptureWindow api={api} initial={state} />
          </I18n>
        </StrictMode>,
      );
    })
  : Promise.all([api.getLanguage(), api.startupState()]).then(async ([language, startup]) => {
      // База повреждена: журнал не открыт, настройки читать не из чего — окно «База повреждена» от них не зависит.
      const settings = startup.needsRecovery || startup.databaseTooNew ? RECOVERY_SETTINGS : await api.getSettings();
      applyTextSize(settings.textSize);
      createRoot(root).render(
        <StrictMode>
          <I18n language={language}>
            <AppRoot api={api} initialSettings={settings} startup={startup} />
          </I18n>
        </StrictMode>,
      );
    });

start.catch((reason: unknown) => {
  // Язык неизвестен, поэтому сообщение без переводов: оно появляется, только если Rust не отвечает совсем.
  root.textContent = `Kinige: ${String((reason as { code?: string } | null)?.code ?? reason)}`;
});
