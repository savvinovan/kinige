import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { INBOX_ID, type KinigeApi, errorCode } from "../../api/KinigeApi";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import type { ChatSummary } from "../../api/generated/ChatSummary";
import type { Settings } from "../../api/generated/Settings";
import type { TextSize } from "../../api/generated/TextSize";
import type { FeedMessage } from "../feed/buildFeed";
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
  const [toast, setToast] = useState<{ messageId: string } | null>(null);
  const [focusSignal, setFocusSignal] = useState(0);
  const closeToast = useCallback(() => setToast(null), []);
  const [settings, setSettings] = useState(initialSettings);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [choosing, setChoosing] = useState(false);
  const [pickError, setPickError] = useState<ErrorCode | null>(null);
  const sending = useRef(false);

  const refreshChats = useCallback(
    () =>
      api
        .listChats()
        .then(setChats)
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
    return () => {
      current = false;
    };
  }, [api]);

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

  const submit = () => {
    if (sending.current || draft.trim() === "") return;
    sending.current = true;
    api
      .sendMessage(INBOX_ID, draft)
      .then((sent) => {
        setMessages((all) => [...all, sent]);
        setDraft("");
        setComposerError(null);
        setToast({ messageId: sent.id });
        void refreshChats();
      })
      // Текст остаётся в поле; запись не показана (без оптимистичного показа).
      .catch((reason: unknown) => setComposerError(errorCode(reason)))
      .finally(() => {
        sending.current = false;
      });
  };

  const undo = (messageId: string) => {
    api
      .undoSend(messageId)
      .then(({ bodyMd }) => {
        setMessages((all) => all.filter((m) => m.id !== messageId));
        // Текст возвращается в поле, чтобы поправить и отправить заново; набранное за 5 секунд не затирается.
        setDraft((current) => (current.trim() === "" ? bodyMd : current));
        setToast(null);
        setFocusSignal((n) => n + 1);
        void refreshChats();
      })
      .catch((reason: unknown) => setComposerError(errorCode(reason)));
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
        setDraft(value);
        setComposerError(null);
      }}
      onSubmit={submit}
      composerError={composerError ? t(`error.${composerError}`) : undefined}
      toast={
        toast
          ? { key: toast.messageId, message: t("toast.sent"), onUndo: () => undo(toast.messageId), onExpire: closeToast }
          : undefined
      }
      composerFocusSignal={focusSignal}
      onOpenLink={openLink}
      onOpenSettings={() => setSettingsOpen(true)}
      overlay={
        settingsOpen ? (
          <SettingsDialog
            settings={settings}
            choosing={choosing}
            actionError={pickError ? t(`error.${pickError}`) : undefined}
            onChooseFolder={chooseFolder}
            onTextSizeChange={changeTextSize}
            onClose={() => {
              setSettingsOpen(false);
              setPickError(null);
            }}
          />
        ) : null
      }
    />
  );
}
