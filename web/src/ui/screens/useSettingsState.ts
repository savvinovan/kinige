import { useCallback, useEffect, useState } from "react";
import type { KinigeApi } from "../../api/KinigeApi";
import type { Settings } from "../../api/generated/Settings";
import { applyTextSize } from "./textSize";

/**
 * Настройки главного окна: размер текста, свёрнутая панель, недавние эмодзи. Сами настройки меняются в отдельном окне
 * «Настройки» (слайс 4b); главное окно узнаёт об этом по событию `settings-changed` (правка окна или файла) и перечитывает
 * настройки, поэтому размер текста меняется сразу, без перезапуска.
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
          // Свёрнутая панель и недавние эмодзи принадлежат этому окну: другое окно их не меняет, берётся то, что у нас.
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
