import { useTranslation } from "react-i18next";
import { ContextMenu, type MenuItem, type MenuPosition } from "../menu/ContextMenu";

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
  onCopy: () => void;
  onClose: () => void;
}

/** Контекстное меню записи: «Копировать», «Редактировать», «Задача», «Избранное», «Изменить дату…», «Удалить» (опасное, за чертой). */
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
  onCopy,
  onClose,
}: MessageMenuProps) {
  const { t } = useTranslation();
  const off = (handler: (() => void) | undefined) => readOnly || !handler;

  const items: MenuItem[] = [
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
    { id: "delete", label: t("message.delete"), icon: "trash", danger: true, separatorBefore: true, disabled: off(onDelete) },
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
      items={items}
      position={position}
      label={t("message.menu")}
      onClose={onClose}
      onSelect={(id) => {
        onClose();
        handlers[id]?.();
      }}
    />
  );
}
