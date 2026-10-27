import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ContextMenu, type MenuItem, type MenuPosition } from "../menu/ContextMenu";
import type { MoveTarget } from "./moveTargets";

interface MessageMenuProps {
  position: MenuPosition;
  isTask: boolean;
  favorite: boolean;
  /** Запись только для чтения: все пункты, кроме «Копировать», недоступны. */
  readOnly: boolean;
  /** Нет обработчика — пункт виден, но недоступен. */
  onEdit?: () => void;
  onToggleTask?: () => void;
  onToggleFavorite?: () => void;
  onChangeDate?: () => void;
  onDelete?: () => void;
  /** Куда можно перенести запись (подменю «Переместить в чат») и что делать при выборе; без них пункт недоступен. */
  moveTargets?: readonly MoveTarget[];
  onMoveTo?: (chatId: string) => void;
  onCopy: () => void;
  onClose: () => void;
}

const CHAT_PREFIX = "chat:";

/**
 * Контекстное меню записи: «Копировать», «Редактировать», «Задача», «Избранное», «Изменить дату…», «Переместить в чат»
 * (подменю со списком чатов), «Удалить» (опасное, за чертой). Подменю — второй уровень в той же панели: «Назад», затем
 * закреплённые, обычные и архивные чаты; текущий чат недоступен. Стрелки и Enter ходят и выбирают, Esc и стрелка влево
 * возвращают на первый уровень.
 */
export function MessageMenu({
  position,
  isTask,
  favorite,
  readOnly,
  onEdit,
  onToggleTask,
  onToggleFavorite,
  onChangeDate,
  onDelete,
  moveTargets = [],
  onMoveTo,
  onCopy,
  onClose,
}: MessageMenuProps) {
  const { t } = useTranslation();
  const [level, setLevel] = useState<"main" | "move">("main");
  const off = (handler: (() => void) | undefined) => readOnly || !handler;
  const canMove = !readOnly && onMoveTo !== undefined && moveTargets.some((target) => !target.current);

  const main: MenuItem[] = [
    { id: "copy", label: t("message.copy"), icon: "copy" },
    { id: "edit", label: t("message.edit"), icon: "edit", disabled: off(onEdit) },
    { id: "task", label: isTask ? t("message.untask") : t("message.task"), icon: "task", disabled: off(onToggleTask) },
    {
      id: "favorite",
      label: favorite ? t("message.unfavorite") : t("message.favorite"),
      icon: "favorite",
      disabled: off(onToggleFavorite),
    },
    { id: "date", label: t("message.changeDate"), icon: "calendar", disabled: off(onChangeDate) },
    { id: "move", label: t("message.moveTo"), icon: "moveTo", disabled: !canMove },
    { id: "delete", label: t("message.delete"), icon: "trash", danger: true, separatorBefore: true, disabled: off(onDelete) },
  ];

  const submenu: MenuItem[] = [
    { id: "back", label: t("message.moveBack"), icon: "chevronLeft" },
    ...moveTargets.map((target, index) => ({
      id: `${CHAT_PREFIX}${target.id}`,
      label: target.name,
      emoji: target.emoji,
      disabled: target.current,
      // Тонкая черта между разделами (закреплённые / чаты / архив) и под «Назад».
      separatorBefore: index === 0 || target.group !== moveTargets[index - 1]?.group,
    })),
  ];

  const handlers: Record<string, (() => void) | undefined> = {
    copy: onCopy,
    edit: onEdit,
    task: onToggleTask,
    favorite: onToggleFavorite,
    date: onChangeDate,
    delete: onDelete,
  };

  return (
    <ContextMenu
      items={level === "main" ? main : submenu}
      position={position}
      label={level === "main" ? t("message.menu") : t("message.moveMenu")}
      levelKey={level}
      onBack={level === "move" ? () => setLevel("main") : undefined}
      onClose={onClose}
      onSelect={(id) => {
        if (id === "move") {
          setLevel("move");
          return;
        }
        if (id === "back") {
          setLevel("main");
          return;
        }
        onClose();
        if (id.startsWith(CHAT_PREFIX)) onMoveTo?.(id.slice(CHAT_PREFIX.length));
        else handlers[id]?.();
      }}
    />
  );
}
