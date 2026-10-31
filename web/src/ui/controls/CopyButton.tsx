import { useCopy } from "../message/useCopy";
import { IconButton } from "./IconButton";

/** Сколько галочка «скопировано» остаётся на кнопке: около полутора секунд. */
export const COPIED_ICON_MS = 1500;

interface CopyButtonProps {
  /** Что кладётся в буфер обмена. */
  text: string;
  /** Подпись и подсказка кнопки: «Копировать путь». */
  label: string;
  /** Подпись, пока на кнопке галочка: «Скопировано». */
  copiedLabel: string;
  /** Подпись, если буфер обмена недоступен: «Не удалось скопировать». */
  failedLabel: string;
  /** Id элемента с копируемым текстом (`aria-describedby`): диктор прочтёт, что именно копируется. */
  describedBy?: string;
}

/**
 * Кнопка «Копировать» значком (меньше шума, чем текстовая): значок копирования, после нажатия на полторы секунды галочка, при
 * сбое буфера обмена — значок ошибки. Подпись и подсказка (`title`) меняются вместе со значком, поэтому диктор слышит
 * «Скопировано». Без анимации и без тостов. Зона нажатия — как у остальных кнопок-значков (`--layout-control-height`).
 */
export function CopyButton({ text, label, copiedLabel, failedLabel, describedBy }: CopyButtonProps) {
  const { state, copy } = useCopy(COPIED_ICON_MS);
  const icon = state === "copied" ? "done" : state === "failed" ? "alert" : "copy";
  const name = state === "copied" ? copiedLabel : state === "failed" ? failedLabel : label;
  return <IconButton icon={icon} label={name} aria-describedby={describedBy} onClick={() => void copy(text)} />;
}
