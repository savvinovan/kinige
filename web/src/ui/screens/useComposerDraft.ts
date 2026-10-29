import { useCallback, useEffect, useRef, useState } from "react";
import { INBOX_ID, type KinigeApi } from "../../api/KinigeApi";
import { useDraftSaver } from "../../draft/useDraftSaver";

/**
 * Текст в поле ввода главного окна и его черновик у каждого чата: подстановка сохранённого при открытии чата, отложенное
 * сохранение при наборе, немедленное — при скрытии окна или потере фокуса (§5.1). Состояние чата (`selected`) хранит
 * `MainWindow`; `switchChat` вызывается в тот же миг, что и его `setSelected`.
 */
export function useComposerDraft(api: KinigeApi, selected: string | null) {
  const [draft, setDraft] = useState("");
  // Пока человек не печатал, черновик из Rust подставляется в пустое поле; набранное до ответа не затирается.
  const typed = useRef(false);
  // Открытый чат без ожидания отрисовки: ответ на `getDraft` прежнего чата, пришедший после переключения, в поле нового не попадает.
  const opened = useRef<string | null>(INBOX_ID);
  const chatId = selected ?? INBOX_ID;

  const save = useCallback((body: string) => api.saveDraft(chatId, body), [api, chatId]);
  const saver = useDraftSaver(save);
  const { flush, schedule } = saver;

  // Номер открытия чата: эффект загрузки черновика идёт заново и тогда, когда `selected` не изменился (A→B→A до отрисовки).
  const [opening, setOpening] = useState(0);

  /**
   * Другой чат: поле пустое, пока не подгрузится его черновик (эффект ниже). Черновик прежнего чата сохраняет вызывающий.
   * Повторный выбор уже открытого чата поле не трогает: набранное остаётся, как и его запланированное сохранение.
   */
  const switchChat = useCallback((next: string | null) => {
    if (opened.current === next) return;
    typed.current = false;
    opened.current = next;
    setDraft("");
    setOpening((count) => count + 1);
  }, []);

  /** Человек печатает: текст в поле, сохранение отложено. */
  const type = useCallback(
    (value: string) => {
      typed.current = true;
      setDraft(value);
      schedule(value);
    },
    [schedule],
  );

  // Черновик открытого чата: пока человек не печатал, подставляется в пустое поле.
  useEffect(() => {
    if (selected === null) return;
    let live = true;
    // Эффект прежнего чата React сбрасывает после отрисовки, а `switchChat` обнуляет поле раньше: одного `live` мало.
    api
      .getDraft(selected)
      .then((saved) => {
        if (live && opened.current === selected && !typed.current && saved !== "") setDraft(saved);
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
    // `opening` в зависимостях: каждое открытие чата перезапускает загрузку, даже если `selected` тот же.
  }, [api, selected, opening]);

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

  return { draft, setDraft, saver, switchChat, type };
}
