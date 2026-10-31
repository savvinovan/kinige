import { useCallback, useEffect, useRef, useState } from "react";
import { INBOX_ID, type KinigeApi } from "../../api/KinigeApi";
import type { ReplyTo } from "../../api/generated/ReplyTo";
import { useDraftSaver } from "../../draft/useDraftSaver";

/** Что уходит в черновик: текст и запись, на которую пишется ответ (слайс 5). */
interface DraftValue {
  body: string;
  replyToId: string | null;
}

/**
 * Текст в поле ввода главного окна, строка «Ответ на запись» и черновик у каждого чата: подстановка сохранённого при открытии
 * чата, отложенное сохранение при наборе, немедленное — при скрытии окна или потере фокуса (§5.1). Состояние чата (`selected`)
 * хранит `MainWindow`; `switchChat` вызывается в тот же миг, что и его `setSelected`.
 */
export function useComposerDraft(api: KinigeApi, selected: string | null) {
  const [draft, setDraft] = useState("");
  const [reply, setReply] = useState<ReplyTo | null>(null);
  // Текст и ответ без ожидания отрисовки: нужны обработчикам, которые меняют что-то одно из двух.
  const latest = useRef<{ body: string; reply: ReplyTo | null }>({ body: "", reply: null });
  // Пока человек не печатал, черновик из Rust подставляется в пустое поле; набранное до ответа не затирается.
  const typed = useRef(false);
  // Открытый чат без ожидания отрисовки: ответ на `getDraft` прежнего чата, пришедший после переключения, в поле нового не попадает.
  const opened = useRef<string | null>(INBOX_ID);
  const chatId = selected ?? INBOX_ID;

  const save = useCallback((value: DraftValue) => api.saveDraft(chatId, value.body, value.replyToId), [api, chatId]);
  const saver = useDraftSaver<DraftValue>(save);
  const { flush, schedule } = saver;

  // Номер открытия чата: эффект загрузки черновика идёт заново и тогда, когда `selected` не изменился (A→B→A до отрисовки).
  const [opening, setOpening] = useState(0);

  const show = useCallback((body: string, replyTo: ReplyTo | null) => {
    latest.current = { body, reply: replyTo };
    setDraft(body);
    setReply(replyTo);
  }, []);

  const plan = useCallback(
    (body: string, replyTo: ReplyTo | null) => schedule({ body, replyToId: replyTo?.id ?? null }),
    [schedule],
  );

  /**
   * Другой чат: поле пустое, пока не подгрузится его черновик (эффект ниже). Черновик прежнего чата сохраняет вызывающий.
   * Повторный выбор уже открытого чата поле не трогает: набранное остаётся, как и его запланированное сохранение.
   */
  const switchChat = useCallback(
    (next: string | null) => {
      if (opened.current === next) return;
      typed.current = false;
      opened.current = next;
      show("", null);
      setOpening((count) => count + 1);
    },
    [show],
  );

  /** Человек печатает: текст в поле, сохранение отложено. */
  const type = useCallback(
    (value: string) => {
      typed.current = true;
      latest.current = { ...latest.current, body: value };
      setDraft(value);
      plan(value, latest.current.reply);
    },
    [plan],
  );

  /** «Ответить» у записи карточки (или крестик строки: `null`): строка над полем меняется, черновик уходит в Rust. */
  const changeReply = useCallback(
    (next: ReplyTo | null) => {
      typed.current = true;
      latest.current = { ...latest.current, reply: next };
      setReply(next);
      plan(latest.current.body, next);
    },
    [plan],
  );

  /** Строка ответа обновилась (после ошибки отправки Rust подтвердил, что записи нет): черновик не пересохраняется. */
  const refreshReply = useCallback((next: ReplyTo | null) => {
    latest.current = { ...latest.current, reply: next };
    setReply(next);
  }, []);

  /** Запись отправлена: поле и строка ответа очищены (Rust уже очистил черновик). */
  const clear = useCallback(() => show("", null), [show]);

  // Черновик открытого чата: пока человек не печатал, подставляется в пустое поле вместе со строкой ответа.
  useEffect(() => {
    if (selected === null) return;
    let live = true;
    // Эффект прежнего чата React сбрасывает после отрисовки, а `switchChat` обнуляет поле раньше: одного `live` мало.
    api
      .getDraft(selected)
      .then((saved) => {
        if (live && opened.current === selected && !typed.current) show(saved.body, saved.replyTo);
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
    // `opening` в зависимостях: каждое открытие чата перезапускает загрузку, даже если `selected` тот же.
  }, [api, selected, opening, show]);

  // Окно скрывается или теряет фокус: черновик уходит в Rust сразу, не дожидаясь паузы в наборе.
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

  return { draft, reply, changeReply, refreshReply, clear, plan, saver, switchChat, type };
}
