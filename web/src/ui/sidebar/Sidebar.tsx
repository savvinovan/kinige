import { TextField } from "../controls/TextField";
import { describeCount, formatCount } from "./formatCount";
import "./sidebar.css";

export interface SidebarChat {
  id: string;
  name: string;
  /** Эмодзи чата — данные пользователя; рисует системный эмодзи-шрифт. */
  emoji: string;
  /** Сколько записей в чате, кроме корзины. 0 или не задано — счётчик скрыт. */
  count?: number;
}

interface SidebarProps {
  chats: readonly SidebarChat[];
  selectedId: string;
  /** В D1 сайдбар статичный (выбор чата и сворачивание — слайс 3): без обработчика строки только показывают выбор. */
  onSelect?: (chatId: string) => void;
}

/**
 * Сайдбар: поле «Поиск» и чаты строками «эмодзи + имя + счётчик записей»; выбранный чат подсвечен.
 * Счётчик справа — тихий серый (не акцент): сколько записей накоплено, а не «что не прочитано». Выбор эмодзи и сворачивание — слайс 3.
 */
export function Sidebar({ chats, selectedId, onSelect }: SidebarProps) {
  return (
    <aside className="sidebar">
      <TextField label="Поиск" icon="search" type="search" placeholder="Поиск" />
      <nav aria-label="Чаты">
        <ul className="sidebar__list">
          {chats.map((chat) => {
            const selected = chat.id === selectedId;
            const counter = formatCount(chat.count ?? 0);
            return (
              <li key={chat.id}>
                <button
                  type="button"
                  className={`chat-row${selected ? " chat-row--selected" : ""}`}
                  aria-current={selected ? "true" : undefined}
                  title={counter ? `${chat.name}, ${describeCount(chat.count ?? 0)}` : chat.name}
                  onClick={onSelect ? () => onSelect(chat.id) : undefined}
                >
                  <span className="chat-row__emoji" aria-hidden="true">
                    {chat.emoji}
                  </span>
                  <span className="chat-row__name">{chat.name}</span>
                  {counter ? (
                    // Пробелы между частями нужны диктору: в имени кнопки они разделяют название, подпись и число.
                    <>
                      {" "}
                      <span className="chat-row__count">
                        <span className="visually-hidden">записей:</span> {counter}
                      </span>
                    </>
                  ) : null}
                </button>
              </li>
            );
          })}
        </ul>
      </nav>
    </aside>
  );
}
