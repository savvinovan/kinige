import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { startDriver } from "virtual:kinige-driver";
import { startProbe } from "virtual:kinige-probe";
import { INBOX_ID } from "./api/KinigeApi";
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

/** One page for three windows: the capture window opens it with `#capture` (`tauri.conf.json`), the settings window with
 * `#settings` (created on first opening), the main one without a hash. */
const isCaptureWindow = window.location.hash === "#capture";
const isSettingsWindow = window.location.hash === "#settings";
// The capture window is transparent (CSS draws the panel shape and shadow): the page needs a marker to remove the background (ui/capture/capture.css).
if (isCaptureWindow) document.documentElement.dataset.window = "capture";
// The settings window: the document does not scroll, only the page on the right does (ui/prefs/prefs.css).
if (isSettingsWindow) document.documentElement.dataset.window = "settings";

// The screens driver (KINIG-92): `undefined` in every normal build (`web/vite.config.ts`). It gets the two Tauri functions the page already
// uses and its own window label; its event and command names live in the driver itself, so a normal bundle holds nothing of it.
startDriver?.({ window: isSettingsWindow ? "settings" : isCaptureWindow ? "capture" : "main", listen, invoke });

// The language and the text size are read before the first frame: the window does not flash a wrong language or size. Rust chooses the language by the system languages.
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
      // The journal is not open or reading the settings itself ran into corruption: the "База повреждена" (Database is corrupted) window (see `loadStartup`).
      const { settings, startup } = await loadStartup(api, first);
      applyTextSize(settings.textSize);
      createRoot(root).render(
        <StrictMode>
          <I18n language={language}>
            <AppRoot api={api} initialSettings={settings} startup={startup} />
          </I18n>
        </StrictMode>,
      );
      // The runners' microphone probe (slice 7a): `undefined` in every normal build (`web/vite.config.ts`).
      startProbe?.((text) => api.sendMessage(INBOX_ID, text));
    });

start.catch((reason: unknown) => {
  // The language is unknown, so the message has no translations: it appears only if Rust does not answer at all.
  root.textContent = `Kinige: ${String((reason as { code?: string } | null)?.code ?? reason)}`;
});
