import { useRef, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import { Icon } from "../icons/Icon";
import { type Route, TREE } from "./routes";
import "./settings-tree.css";

interface SettingsTreeProps {
  route: Route;
  onNavigate: (route: Route) => void;
  /** Разделы, у которых есть ошибка: рядом с названием метка (пока это «Файл настроек»). */
  withProblems?: readonly Route[];
}

/**
 * Дерево разделов окна «Настройки»: `nav` со списком кнопок. Выбранный пункт — `aria-current="page"` и заливка; вложенные
 * пункты («Видимые чаты», «Журнал обращений») стоят под «Доступом ИИ-агентов» с отступом. ↑ и ↓ переносят фокус на соседний
 * пункт, Enter (или пробел) открывает страницу, Home и End — первый и последний пункт. В обходе Tab участвует один пункт,
 * выбранный (roving tabindex). Метка ошибки — значок (для диктора слово «есть проблемы» в подписи пункта), а не один цвет.
 */
export function SettingsTree({ route, onNavigate, withProblems = [] }: SettingsTreeProps) {
  const { t } = useTranslation();
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);

  const onKeyDown = (event: KeyboardEvent<HTMLUListElement>) => {
    const index = buttons.current.findIndex((button) => button === document.activeElement);
    if (index < 0) return;
    const last = TREE.length - 1;
    const target =
      event.key === "ArrowDown" ? Math.min(index + 1, last) : event.key === "ArrowUp" ? Math.max(index - 1, 0) : event.key === "Home" ? 0 : event.key === "End" ? last : null;
    if (target === null) return;
    event.preventDefault();
    buttons.current[target]?.focus();
  };

  return (
    <nav className="settings-tree" aria-label={t("prefs.treeLabel")}>
      <ul className="settings-tree__list" onKeyDown={onKeyDown}>
        {TREE.map((item, index) => {
          const current = item.route === route;
          const problem = withProblems.includes(item.route);
          return (
            <li key={item.route} className="settings-tree__item">
              <button
                type="button"
                ref={(node) => {
                  buttons.current[index] = node;
                }}
                className={`settings-tree__button${item.level === 1 ? " settings-tree__button--nested" : ""}`}
                aria-current={current ? "page" : undefined}
                aria-label={problem ? `${t(item.label)}, ${t("prefs.errorMark")}` : undefined}
                tabIndex={current ? 0 : -1}
                onClick={() => onNavigate(item.route)}
              >
                <span className="settings-tree__label">{t(item.label)}</span>
                {problem ? (
                  <span className="settings-tree__mark">
                    <Icon name="alert" />
                  </span>
                ) : null}
              </button>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
