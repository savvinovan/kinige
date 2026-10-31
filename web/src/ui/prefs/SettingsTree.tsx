import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import { Icon } from "../icons/Icon";
import { GROUPS, type Route, TREE, groupOf, parentOf } from "./routes";
import "./settings-tree.css";

/** Где окно помнит, какие группы дерева развёрнуты, между запусками (только это окно, не `settings.json`). */
const STORAGE_KEY = "kinige.settingsTree.expanded";

function loadExpanded(): Route[] {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "[]");
    return Array.isArray(parsed) ? parsed.filter((item): item is Route => typeof item === "string" && item in GROUPS) : [];
  } catch {
    // Хранилища нет или значение повреждено: все группы свёрнуты.
    return [];
  }
}

function saveExpanded(groups: ReadonlySet<Route>) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify([...groups]));
  } catch {
    // Хранилище недоступно: состояние живёт до закрытия окна.
  }
}

interface SettingsTreeProps {
  route: Route;
  onNavigate: (route: Route) => void;
  /** Пункты, у которых есть ошибка: рядом с названием метка (пока это «Файл настроек»). У свёрнутой группы — на разделе. */
  withProblems?: readonly Route[];
  /** Развёрнутые группы при открытии (истории Storybook); без него берётся сохранённое состояние. */
  defaultExpanded?: readonly Route[];
}

/**
 * Дерево разделов окна «Настройки»: `nav` со списком кнопок. Выбранный пункт — `aria-current="page"` и заливка (без рамки);
 * вложенные пункты («Видимые чаты», «Журнал обращений») стоят под «Доступом ИИ-агентов» и сворачиваются: шеврон слева
 * сворачивает и разворачивает группу, название открывает раздел и разворачивает группу. По умолчанию группы свёрнуты, кроме
 * той, где текущая страница (она разворачивается сама при любом переходе); свёрнутая группа с текущим подпунктом подсвечена.
 * Состояние помнится на время жизни окна и в `localStorage`.
 *
 * Клавиатура по образцу WAI-ARIA tree: ↑ и ↓ — по видимым пунктам, → разворачивает группу (на развёрнутой — переходит к первому
 * вложенному), ← сворачивает (на вложенном — переходит к разделу), Home и End — крайние пункты, Enter и пробел открывают
 * страницу. `aria-expanded` стоит на разделе с вложенными пунктами. В обходе Tab один пункт — выбранный, а если он скрыт в
 * свёрнутой группе, то её раздел. Кольцо фокуса — только `:focus-visible` (с клавиатуры). Движения нет.
 */
export function SettingsTree({ route, onNavigate, withProblems = [], defaultExpanded }: SettingsTreeProps) {
  const { t } = useTranslation();
  const list = useRef<HTMLUListElement>(null);
  const [expanded, setExpanded] = useState<ReadonlySet<Route>>(() => {
    const open = new Set<Route>(defaultExpanded ?? loadExpanded());
    const current = groupOf(route);
    if (current) open.add(current);
    return open;
  });

  // Любой переход (дерево, «Настроить ›», «назад», ошибка файла) разворачивает группу текущей страницы. Состояние
  // подстраивается при отрисовке, а не в эффекте: без лишнего кадра со свёрнутой группой.
  const [seenRoute, setSeenRoute] = useState(route);
  if (seenRoute !== route) {
    setSeenRoute(route);
    const group = groupOf(route);
    if (group && !expanded.has(group)) setExpanded(new Set(expanded).add(group));
  }

  useEffect(() => {
    if (!defaultExpanded) saveExpanded(expanded);
  }, [expanded, defaultExpanded]);

  const toggle = (group: Route, open: boolean) =>
    setExpanded((old) => {
      const next = new Set(old);
      if (open) next.add(group);
      else next.delete(group);
      return next;
    });

  const buttons = () => [...(list.current?.querySelectorAll<HTMLButtonElement>("button[data-route]") ?? [])];
  const focusRoute = (target: Route) => buttons().find((button) => button.dataset.route === target)?.focus();

  const onKeyDown = (event: KeyboardEvent<HTMLUListElement>) => {
    const items = buttons();
    const index = items.findIndex((button) => button === document.activeElement);
    const current = items[index];
    if (!current) return;
    const at = current.dataset.route as Route;
    const children = GROUPS[at];
    const open = expanded.has(at);
    let handled = true;
    switch (event.key) {
      case "ArrowDown":
        items[Math.min(index + 1, items.length - 1)]?.focus();
        break;
      case "ArrowUp":
        items[Math.max(index - 1, 0)]?.focus();
        break;
      case "Home":
        items[0]?.focus();
        break;
      case "End":
        items[items.length - 1]?.focus();
        break;
      case "ArrowRight":
        if (children && !open) toggle(at, true);
        else if (children?.[0]) focusRoute(children[0]);
        break;
      case "ArrowLeft":
        if (children && open) toggle(at, false);
        else if (parentOf(at) !== at) focusRoute(parentOf(at));
        break;
      default:
        handled = false;
    }
    if (handled) event.preventDefault();
  };

  const hidden = (item: Route) => {
    const group = groupOf(item);
    return group !== undefined && group !== item && !expanded.has(group);
  };
  // Единственный пункт в обходе Tab: выбранный, а если он скрыт в свёрнутой группе — раздел этой группы.
  const tabStop = hidden(route) ? parentOf(route) : route;

  return (
    <nav className="settings-tree" aria-label={t("prefs.treeLabel")}>
      <ul className="settings-tree__list" ref={list} onKeyDown={onKeyDown}>
        {TREE.filter((item) => !hidden(item.route)).map((item) => {
          const children = GROUPS[item.route];
          const open = expanded.has(item.route);
          const current = item.route === route;
          // Свёрнутая группа с текущим подпунктом подсвечена; метка ошибки подпункта видна на разделе, пока группа свёрнута.
          const containsCurrent = !!children && !open && parentOf(route) === item.route && !current;
          const problem =
            withProblems.includes(item.route) || (!!children && !open && children.some((child) => withProblems.includes(child)));
          return (
            <li key={item.route} className="settings-tree__item">
              <div
                className={`settings-tree__row${item.level === 1 ? " settings-tree__row--nested" : ""}`}
                data-current={current || undefined}
                data-contains-current={containsCurrent || undefined}
              >
                {children ? (
                  <span
                    className="settings-tree__chevron"
                    aria-hidden="true"
                    data-testid={`tree-chevron-${item.route}`}
                    onClick={() => toggle(item.route, !open)}
                  >
                    <Icon name={open ? "chevron" : "chevronRight"} />
                  </span>
                ) : (
                  <span className="settings-tree__chevron" aria-hidden="true" />
                )}
                <button
                  type="button"
                  data-route={item.route}
                  className="settings-tree__button"
                  aria-current={current ? "page" : undefined}
                  aria-expanded={children ? open : undefined}
                  aria-label={problem ? `${t(item.label)}, ${t("prefs.errorMark")}` : undefined}
                  tabIndex={item.route === tabStop ? 0 : -1}
                  onClick={() => onNavigate(item.route)}
                >
                  <span className="settings-tree__label">{t(item.label)}</span>
                  {problem ? (
                    <span className="settings-tree__mark">
                      <Icon name="alert" />
                    </span>
                  ) : null}
                </button>
              </div>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
