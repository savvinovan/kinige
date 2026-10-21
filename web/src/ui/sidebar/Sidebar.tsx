import { useTranslation } from "react-i18next";
import type { ChatSummary } from "../../api/generated/ChatSummary";
import { useLanguage } from "../../i18n/I18n";
import { TextField } from "../controls/TextField";
import { formatCount } from "./formatCount";
import { ProfileMenu } from "./ProfileMenu";
import "./sidebar.css";

export interface SidebarChat {
  id: string;
  /** Название чата — данные пользователя. У Inbox (`isInbox`) хранимое имя пустое, интерфейс показывает переведённое. */
  name: string;
  /** Inbox: пустое имя заменяется переводом «Входящие» / «Inbox». */
  isInbox?: boolean;
  /** Эмодзи чата — данные пользователя; рисует системный эмодзи-шрифт. */
  emoji: string;
  /** Сколько записей в чате, кроме корзины. 0 или не задано — счётчик скрыт. */
  count?: number;
}

/** Чат из ответа Rust → строка сайдбара. Чат без эмодзи получает нейтральный значок. */
export function toSidebarChat(chat: ChatSummary): SidebarChat {
  return { id: chat.id, name: chat.title, isInbox: chat.isInbox, emoji: chat.icon ?? "💬", count: chat.count };
}

interface SidebarProps {
  chats: readonly SidebarChat[];
  selectedId: string;
  /** В D1 сайдбар статичный (выбор чата и сворачивание — слайс 3): без обработчика строки только показывают выбор. */
  onSelect?: (chatId: string) => void;
  /** Строка профиля внизу: имя (по умолчанию «Я») и точка статуса у аватара (слайс 11; в D1 не показывается). */
  profile?: { name?: string; statusDot?: boolean };
  /** «Настройки» в меню профиля (слайс 1). */
  onOpenSettings?: () => void;
  /** Поле «Поиск». В приложении его нет до слайса 4 (мёртвых полей нет, §5); в историях D1 оно нарисовано. */
  showSearch?: boolean;
}

/**
 * Сайдбар: поле «Поиск» и чаты строками «эмодзи + имя + счётчик записей»; выбранный чат подсвечен.
 * Счётчик справа — тихий серый (не акцент): сколько записей накоплено, а не «что не прочитано». Выбор эмодзи и сворачивание — слайс 3.
 */
export function Sidebar({ chats, selectedId, onSelect, profile, onOpenSettings, showSearch = true }: SidebarProps) {
  const { t } = useTranslation();
  const language = useLanguage();
  return (
    <aside className="sidebar">
      {showSearch ? <TextField label={t("sidebar.search")} icon="search" type="search" placeholder={t("sidebar.search")} /> : null}
      <nav aria-label={t("sidebar.chats")}>
        <ul className="sidebar__list">
          {chats.map((chat) => {
            const selected = chat.id === selectedId;
            const counter = formatCount(chat.count ?? 0, language);
            const name = chat.isInbox && chat.name === "" ? t("chat.inbox") : chat.name;
            return (
              <li key={chat.id}>
                <button
                  type="button"
                  className={`chat-row${selected ? " chat-row--selected" : ""}`}
                  aria-current={selected ? "true" : undefined}
                  title={counter ? `${name}, ${t("count.entries", { count: chat.count ?? 0 })}` : name}
                  onClick={onSelect ? () => onSelect(chat.id) : undefined}
                >
                  <span className="chat-row__emoji" aria-hidden="true">
                    {chat.emoji}
                  </span>
                  <span className="chat-row__name">{name}</span>
                  {counter ? (
                    // Диктору читается «Рецепты, записей: 12 тыс.»: запятая и подпись скрыты визуально, число видно.
                    <span className="chat-row__count">
                      <span className="visually-hidden">{t("sidebar.entriesLabel")}</span> {counter}
                    </span>
                  ) : null}
                </button>
              </li>
            );
          })}
        </ul>
      </nav>
      <ProfileMenu name={profile?.name} statusDot={profile?.statusDot} onOpenSettings={onOpenSettings} />
    </aside>
  );
}
