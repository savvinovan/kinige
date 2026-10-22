import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { INBOX_ID, type KinigeApi, errorCode } from "../../api/KinigeApi";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import type { ChatSummary } from "../../api/generated/ChatSummary";
import type { Settings } from "../../api/generated/Settings";
import type { ShortcutKind } from "../../api/generated/ShortcutKind";
import type { TextSize } from "../../api/generated/TextSize";
import { useDraftSaver } from "../../draft/useDraftSaver";
import type { FeedMessage } from "../feed/buildFeed";
import { mergeLatestPage } from "../feed/mergeFeed";
import { SettingsDialog } from "../settings/SettingsDialog";
import { toSidebarChat } from "../sidebar/Sidebar";
import { MainWindowView } from "./MainWindowView";

/** Сколько записей грузит первая страница ленты; более старые (дозагрузка) — слайс 8. */
const PAGE_SIZE = 200;

/** Размер текста окна: атрибут на `<html>` читают токены (`data-text-scale`, ADR-0004). */
export function applyTextSize(size: TextSize): void {
  const root = document.documentElement;
  if (size === "large") root.setAttribute("data-text-scale", "large");
  else root.removeAttribute("data-text-scale");
}

interface MainWindowProps {
  api: KinigeApi;
  /** Настройки, прочитанные до первого кадра (размер текста применяется без мигания). */
  initialSettings: Settings;
}

/**
 * Главное окно слайса 1 на настоящем `KinigeApi`: Inbox с счётчиком, лента страницей из Rust, поле ввода, тост «Отменить»,
 * настройки. Без оптимистичного показа (§5): запись появляется в ленте после ответа Rust; при ошибке текст остаётся в поле,
 * над ним — ошибка.
 */
export function MainWindow({ api, initialSettings }: MainWindowProps) {
  const { t } = useTranslation();
  const [chats, setChats] = useState<ChatSummary[]>([]);
  const [messages, setMessages] = useState<FeedMessage[]>([]);
  const [today, setToday] = useState("");
  const [loadError, setLoadError] = useState<ErrorCode | null>(null);
  const [draft, setDraft] = useState("");
  const [composerError, setComposerError] = useState<ErrorCode | null>(null);
  const [settings, setSettings] = useState(initialSettings);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [choosing, setChoosing] = useState(false);
  const [pickError, setPickError] = useState<ErrorCode | null>(null);
  const [shortcutErrors, setShortcutErrors] = useState<Partial<Record<ShortcutKind, string>>>({});
  const sending = useRef(false);
  // Пока человек не печатал, черновик из Rust подставляется в пустое поле; набранное до ответа не затирается.
  const typed = useRef(false);

  const saveDraft = useCallback((body: string) => api.saveDraft(INBOX_ID, body), [api]);
  const draftSaver = useDraftSaver(saveDraft);

  const refreshChats = useCallback(
    () =>
      api
        .listChats()
        .then(setChats)
        .catch(() => undefined),
    [api],
  );

  // Свежая последняя страница и счётчик: запись пришла из окна захвата или из буфера (событие Rust «записи изменились»).
  const reload = useCallback(
    () =>
      Promise.all([api.listChats(), api.listMessages(INBOX_ID, { limit: PAGE_SIZE })])
        .then(([loadedChats, page]) => {
          setChats(loadedChats);
          // Подгруженная история не сбрасывается: страница сливается с уже загруженным (см. `mergeLatestPage`).
          setMessages((loaded) => mergeLatestPage(loaded, page.messages, page.older !== null));
          setToday(page.today);
        })
        .catch(() => undefined),
    [api],
  );

  useEffect(() => {
    let current = true;
    Promise.all([api.listChats(), api.listMessages(INBOX_ID, { limit: PAGE_SIZE })])
      .then(([loadedChats, page]) => {
        if (!current) return;
        setChats(loadedChats);
        setMessages(page.messages);
        setToday(page.today);
      })
      .catch((reason: unknown) => current && setLoadError(errorCode(reason)));
    api
      .getDraft(INBOX_ID)
      .then((saved) => {
        if (current && !typed.current && saved !== "") setDraft(saved);
      })
      .catch(() => undefined);
    return () => {
      current = false;
    };
  }, [api]);

  useEffect(() => {
    let off: (() => void) | undefined;
    let closed = false;
    api
      .onEntriesChanged(() => void reload())
      .then((unlisten) => {
        if (closed) unlisten();
        else off = unlisten;
      })
      .catch(() => undefined);
    return () => {
      closed = true;
      off?.();
    };
  }, [api, reload]);

  // Окно скрывается или теряет фокус: черновик уходит в Rust сразу, не дожидаясь паузы в наборе (§5.1).
  const { flush } = draftSaver;
  useEffect(() => {
    const onHide = () => {
      if (document.visibilityState === "hidden") void flush();
    };
    const onBlur = () => void flush();
    document.addEventListener("visibilitychange", onHide);
    window.addEventListener("blur", onBlur);
    return () => {
      document.removeEventListener("visibilitychange", onHide);
      window.removeEventListener("blur", onBlur);
    };
  }, [flush]);

  useEffect(() => applyTextSize(settings.textSize), [settings.textSize]);

  // Пока окно настроек открыто, состояние копии (ошибка, число дней в очереди) обновляется само.
  useEffect(() => {
    if (!settingsOpen) return;
    const timer = setInterval(() => {
      api
        .getSettings()
        .then((fresh) => setSettings((old) => ({ ...old, mirror: fresh.mirror })))
        .catch(() => undefined);
    }, 2000);
    return () => clearInterval(timer);
  }, [api, settingsOpen]);

  // Окно настроек открывается со свежим состоянием копии, а не со снимком при старте приложения: иначе ошибка папки
  // или очередь дней показались бы как «Копия актуальна». Не удалось прочитать — открываем с тем, что есть.
  const openSettings = () => {
    api
      .getSettings()
      .then((fresh) => setSettings((old) => ({ ...old, mirror: fresh.mirror, shortcuts: fresh.shortcuts })))
      .catch(() => undefined)
      .finally(() => setSettingsOpen(true));
  };

  const submit = () => {
    if (sending.current || draft.trim() === "") return;
    sending.current = true;
    // Пока идёт отправка, ввод не принимается (см. `onDraftChange`), а запланированное сохранение черновика отменено, уже
    // отправленное — дожидаемся: Rust очищает черновик чата в той же транзакции, что и отправку, а запоздавшая запись
    // вернула бы отправленный текст.
    draftSaver.cancel();
    const text = draft;
    draftSaver
      .settle()
      .then(() => api.sendMessage(INBOX_ID, text))
      .then((sent) => {
        draftSaver.cancel();
        setMessages((all) => [...all, sent]);
        setDraft("");
        setComposerError(null);
        void refreshChats();
      })
      // Успех: запись просто появляется в ленте, без тоста и подтверждений (как в Telegram). Ошибка: текст остаётся в поле,
      // запись не показана (без оптимистичного показа); черновик возвращается в очередь сохранения.
      .catch((reason: unknown) => {
        setComposerError(errorCode(reason));
        draftSaver.schedule(text);
      })
      .finally(() => {
        sending.current = false;
      });
  };

  const openLink = (url: string) => {
    api.openExternalUrl(url).catch((reason: unknown) => setComposerError(errorCode(reason)));
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

  if (loadError) return <p role="alert">{`${t("error.loadFailed")}: ${t(`error.${loadError}`)}`}</p>;

  return (
    <MainWindowView
      interactive
      showSearch={false}
      composerButtons={{}}
      chats={chats.map(toSidebarChat)}
      selectedChatId={INBOX_ID}
      messages={messages}
      today={today}
      draft={draft}
      onDraftChange={(value) => {
        if (sending.current) return;
        typed.current = true;
        setDraft(value);
        setComposerError(null);
        draftSaver.schedule(value);
      }}
      onSubmit={submit}
      composerError={composerError ? t(`error.${composerError}`) : undefined}
      onOpenLink={openLink}
      onOpenSettings={openSettings}
      overlay={
        settingsOpen ? (
          <SettingsDialog
            settings={settings}
            shortcuts={settings.shortcuts}
            onShortcutChange={(kind, accelerator) => applyShortcut(kind, api.setShortcut(kind, accelerator))}
            onShortcutReset={(kind) => applyShortcut(kind, api.resetShortcut(kind))}
            onShortcutRecording={(active) => void api.shortcutRecording(active).catch(() => undefined)}
            shortcutErrors={shortcutErrors}
            choosing={choosing}
            actionError={pickError ? t(`error.${pickError}`) : undefined}
            onChooseFolder={chooseFolder}
            onTextSizeChange={changeTextSize}
            onClose={() => {
              setSettingsOpen(false);
              setPickError(null);
              setShortcutErrors({});
            }}
          />
        ) : null
      }
    />
  );
}
