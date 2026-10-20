import { TextField } from "../controls/TextField";
import "./sidebar.css";

export interface SidebarChat {
  id: string;
  name: string;
  /** Эмодзи чата — данные пользователя; рисует системный эмодзи-шрифт. */
  emoji: string;
}

interface SidebarProps {
  chats: readonly SidebarChat[];
  selectedId: string;
  /** В D1 сайдбар статичный (выбор чата и сворачивание — слайс 3): без обработчика строки только показывают выбор. */
  onSelect?: (chatId: string) => void;
}

/** Сайдбар: поле «Поиск» и чаты строками «эмодзи + имя»; выбранный чат подсвечен. Выбор эмодзи и сворачивание — слайс 3. */
export function Sidebar({ chats, selectedId, onSelect }: SidebarProps) {
  return (
    <aside className="sidebar">
      <TextField label="Поиск" icon="search" type="search" placeholder="Поиск" />
      <nav aria-label="Чаты">
        <ul className="sidebar__list">
          {chats.map((chat) => {
            const selected = chat.id === selectedId;
            return (
              <li key={chat.id}>
                <button
                  type="button"
                  className={`chat-row${selected ? " chat-row--selected" : ""}`}
                  aria-current={selected ? "true" : undefined}
                  title={chat.name}
                  onClick={onSelect ? () => onSelect(chat.id) : undefined}
                >
                  <span className="chat-row__emoji" aria-hidden="true">
                    {chat.emoji}
                  </span>
                  <span className="chat-row__name">{chat.name}</span>
                </button>
              </li>
            );
          })}
        </ul>
      </nav>
    </aside>
  );
}
