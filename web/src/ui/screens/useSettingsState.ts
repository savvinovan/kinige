import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { type KinigeApi, errorCode } from "../../api/KinigeApi";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import type { Settings } from "../../api/generated/Settings";
import type { ShortcutKind } from "../../api/generated/ShortcutKind";
import type { TextSize } from "../../api/generated/TextSize";
import { applyTextSize } from "./textSize";

/** Настройки главного окна: размер текста, папка копии, сочетания, свёрнутая панель, недавние эмодзи, окно настроек. */
export function useSettingsState(api: KinigeApi, initialSettings: Settings) {
  const { t } = useTranslation();
  const [settings, setSettings] = useState(initialSettings);
  const [open, setOpen] = useState(false);
  const [choosing, setChoosing] = useState(false);
  const [pickError, setPickError] = useState<ErrorCode | null>(null);
  const [shortcutErrors, setShortcutErrors] = useState<Partial<Record<ShortcutKind, string>>>({});

  useEffect(() => applyTextSize(settings.textSize), [settings.textSize]);

  // Пока окно настроек открыто, состояние копий (ошибка, число дней в очереди, последняя копия) обновляется само.
  useEffect(() => {
    if (!open) return;
    const timer = setInterval(() => {
      api
        .getSettings()
        .then((fresh) => setSettings((old) => ({ ...old, mirror: fresh.mirror, backup: fresh.backup })))
        .catch(() => undefined);
    }, 2000);
    return () => clearInterval(timer);
  }, [api, open]);

  // Окно настроек открывается со свежим состоянием копий, а не со снимком при старте приложения.
  const openDialog = () => {
    api
      .getSettings()
      .then((fresh) =>
        setSettings((old) => ({ ...old, mirror: fresh.mirror, shortcuts: fresh.shortcuts, backup: fresh.backup })),
      )
      .catch(() => undefined)
      .finally(() => setOpen(true));
  };

  const closeDialog = () => {
    setOpen(false);
    setPickError(null);
    setShortcutErrors({});
  };

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

  const chooseFolder = () => {
    setChoosing(true);
    setPickError(null);
    api
      .chooseMirrorFolder()
      .then(setSettings)
      .catch((reason: unknown) => setPickError(errorCode(reason)))
      .finally(() => setChoosing(false));
  };

  // Смена или сброс сочетания: Rust проверяет запись, регистрирует в системе и сохраняет; отказ — код ошибки под строкой.
  const applyShortcut = (kind: ShortcutKind, request: Promise<Settings>) => {
    setShortcutErrors((old) => ({ ...old, [kind]: undefined }));
    request
      .then(setSettings)
      .catch((reason: unknown) => setShortcutErrors((old) => ({ ...old, [kind]: t(`error.${errorCode(reason)}`) })));
  };

  const changeTextSize = (size: TextSize) => {
    api
      .setTextSize(size)
      .then(() => setSettings((old) => ({ ...old, textSize: size })))
      .catch((reason: unknown) => setPickError(errorCode(reason)));
  };

  return {
    settings,
    open,
    choosing,
    pickError,
    shortcutErrors,
    openDialog,
    closeDialog,
    recordEmoji,
    toggleCollapsed,
    chooseFolder,
    changeTextSize,
    changeShortcut: (kind: ShortcutKind, accelerator: string) => applyShortcut(kind, api.setShortcut(kind, accelerator)),
    resetShortcut: (kind: ShortcutKind) => applyShortcut(kind, api.resetShortcut(kind)),
  };
}
