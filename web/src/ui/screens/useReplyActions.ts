import { useCallback, useEffect, useRef } from "react";
import { type KinigeApi, errorCode } from "../../api/KinigeApi";
import type { ErrorCode } from "../../api/generated/ErrorCode";
import type { MessageView } from "../../api/generated/MessageView";
import type { OnThisDayEntry } from "../../api/generated/OnThisDayEntry";
import type { ReplyTo } from "../../api/generated/ReplyTo";
import type { DraftSaver } from "../../draft/useDraftSaver";
import { focusComposer } from "./mainWindowParts";
import type { FocusTarget } from "./useFeed";

interface UseReplyActionsOptions {
  api: KinigeApi;
  /** Открытый чат; `null` — корзина. */
  selected: string | null;
  select: (chatId: string | null) => Promise<void>;
  focusOn: (target: FocusTarget) => void;
  draft: string;
  reply: ReplyTo | null;
  saver: DraftSaver<{ body: string; replyToId: string | null }>;
  changeReply: (reply: ReplyTo | null) => void;
  refreshReply: (reply: ReplyTo | null) => void;
  clearDraft: () => void;
  planDraft: (body: string, reply: ReplyTo | null) => void;
  appendSent: (sent: MessageView) => void;
  refreshChats: () => Promise<void>;
  setComposerError: (code: ErrorCode | null) => void;
  dismissCard: () => Promise<void>;
  /** Карточка скрыта: лента возвращается к нижнему краю. */
  onDismissed: () => void;
}

/**
 * Отправка записи (в том числе ответом) и действия слайса 5: «Ответить» у записи карточки, переход к записи из цитаты и
 * карточки, крестик карточки. Без оптимистичного показа (§5): запись появляется после ответа Rust, при ошибке текст остаётся
 * в поле, над ним — ошибка.
 */
export function useReplyActions(options: UseReplyActionsOptions) {
  const { api, selected, select, focusOn, draft, reply, saver, changeReply, refreshReply, clearDraft, planDraft, refreshChats } = options;
  const { appendSent, setComposerError, dismissCard, onDismissed } = options;
  const sending = useRef(false);
  const selectedRef = useRef(selected);
  useEffect(() => {
    selectedRef.current = selected;
  });

  /** Черновик с ответом уходит в Rust, и строка над полем берёт состояние записи оттуда. */
  const confirmReplyState = useCallback(
    async (chatId: string) => {
      await saver.flush();
      const saved = await api.getDraft(chatId).catch(() => null);
      if (saved !== null && selectedRef.current === chatId) refreshReply(saved.replyTo);
    },
    [api, saver, refreshReply],
  );

  const submit = () => {
    if (sending.current || draft.trim() === "" || selected === null) return;
    sending.current = true;
    // Пока идёт отправка, ввод не принимается (см. `onDraftChange`), а запланированное сохранение черновика отменено, уже
    // отправленное — дожидаемся: Rust очищает черновик чата в той же транзакции, что и отправку, а запоздавшая запись
    // вернула бы отправленный текст.
    saver.cancel();
    const text = draft;
    const replyTo = reply;
    // Запись, на которую отвечали, перенесли в другой чат: строка над полем уже сказала об этом, запись уходит без ответа
    // (Rust принимает ответ только на запись того же чата).
    const replyId = replyTo === null || replyTo.state === "moved" ? null : replyTo.id;
    saver
      .settle()
      .then(() => api.sendMessage(selected, text, replyId))
      .then((sent) => {
        saver.cancel();
        appendSent(sent);
        clearDraft();
        setComposerError(null);
        void refreshChats();
      })
      // Успех: запись просто появляется в ленте, без тоста и подтверждений (как в Telegram). Ошибка: текст остаётся в поле,
      // запись не показана; черновик возвращается в очередь сохранения. Если Rust сказал, что записи, на которую отвечают,
      // нет, строка над полем перечитывается: она покажет «в корзине», «удалена» или «перенесена».
      .catch((reason: unknown) => {
        const code = errorCode(reason);
        setComposerError(code);
        planDraft(text, replyTo);
        if (code === "reply_target_not_found") void confirmReplyState(selected);
      })
      .finally(() => {
        sending.current = false;
      });
  };

  /** Открыть запись в ленте в контексте (чат и запись): как переход из поиска, запись подсвечена заливкой. */
  const openEntry = (chatId: string, messageId: string) => {
    void select(chatId).then(() => focusOn({ chatId, messageId, terms: [], nonce: 0 }));
  };

  /** «Ответить» у записи карточки: строка над полем и курсор в поле. */
  const startReply = (entry: OnThisDayEntry) => {
    changeReply({
      id: entry.message.id,
      chatId: entry.message.chatId,
      journalDate: entry.message.journalDate,
      excerpt: entry.text,
      state: "live",
      yearsLater: 0,
    });
    focusComposer();
  };

  const hideCard = () => {
    void dismissCard().then(() => {
      focusComposer();
      onDismissed();
    });
  };

  return { sending, submit, openEntry, startReply, hideCard };
}
