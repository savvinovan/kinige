import { useCallback, useEffect, useState } from "react";
import type { KinigeApi } from "../../api/KinigeApi";
import type { Settings } from "../../api/generated/Settings";
import { applyTextSize } from "./textSize";

/**
 * Settings of the main window: text size, collapsed sidebar, recent emoji. The settings themselves are changed in the
 * separate "Settings" window (slice 4b); the main window learns about it from the `settings-changed` event (an edit in
 * the window or in the file) and rereads the settings, so the text size changes at once, without a restart.
 */
export function useSettingsState(api: KinigeApi, initialSettings: Settings) {
  const [settings, setSettings] = useState(initialSettings);

  useEffect(() => applyTextSize(settings.textSize), [settings.textSize]);

  useEffect(() => {
    let closed = false;
    let off: (() => void) | undefined;
    api
      .onSettingsChanged(() => {
        api
          .getSettings()
          // The collapsed sidebar and recent emoji belong to this window: no other window changes them, so we keep ours.
          .then((fresh) => setSettings((old) => ({ ...fresh, sidebarCollapsed: old.sidebarCollapsed, recentEmoji: old.recentEmoji })))
          .catch(() => undefined);
      })
      .then((unlisten) => {
        if (closed) unlisten();
        else off = unlisten;
      })
      .catch(() => undefined);
    return () => {
      closed = true;
      off?.();
    };
  }, [api]);

  const recordEmoji = useCallback(
    (emoji: string) => {
      api
        .addRecentEmoji(emoji)
        .then((recentEmoji) => setSettings((old) => ({ ...old, recentEmoji })))
        .catch(() => undefined);
    },
    [api],
  );

  const toggleCollapsed = () => {
    const collapsed = !settings.sidebarCollapsed;
    setSettings((old) => ({ ...old, sidebarCollapsed: collapsed }));
    api.setSidebarCollapsed(collapsed).catch(() => undefined);
  };

  return { settings, recordEmoji, toggleCollapsed };
}
