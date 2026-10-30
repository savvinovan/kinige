import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { TauriApi } from "./api/TauriApi";
import "./app.css";
import { CaptureWindow } from "./capture/CaptureWindow";
import { I18n } from "./i18n/I18n";
import { SettingsWindow } from "./settings/SettingsWindow";
import { loadPreferences } from "./settings/usePreferences";
import { AppRoot } from "./ui/screens/AppRoot";
import { loadStartup } from "./ui/screens/startupSettings";
import { applyTextSize } from "./ui/screens/textSize";

const root = document.getElementById("root");
if (!root) throw new Error("no #root element");

const api = new TauriApi();

/** Одна страница на три окна: окно захвата открывает её с `#capture` (`tauri.conf.json`), окно настроек — с `#settings`
 * (создаётся при первом открытии), главное — без хэша. */
const isCaptureWindow = window.location.hash === "#capture";
const isSettingsWindow = window.location.hash === "#settings";
// Окно захвата прозрачное (форму и тень панели рисует CSS): странице нужна метка, чтобы убрать фон (ui/capture/capture.css).
if (isCaptureWindow) document.documentElement.dataset.window = "capture";

// Язык и размер текста читаются до первого кадра: окно не мигает чужим языком и размером. Язык выбирает Rust по языкам системы.
const start = isSettingsWindow
  ? Promise.all([api.getLanguage(), loadPreferences(api)]).then(([language, initial]) => {
      applyTextSize(initial.settings.textSize);
      createRoot(root).render(
        <StrictMode>
          <I18n language={language}>
            <SettingsWindow api={api} initial={initial} />
          </I18n>
        </StrictMode>,
      );
    })
  : isCaptureWindow
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
  : Promise.all([api.getLanguage(), api.startupState()]).then(async ([language, first]) => {
      // Журнал не открыт или чтение настроек само упёрлось в повреждение: окно «База повреждена» (см. `loadStartup`).
      const { settings, startup } = await loadStartup(api, first);
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
