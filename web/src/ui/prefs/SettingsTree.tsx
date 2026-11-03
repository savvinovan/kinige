import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";
import { Icon } from "../icons/Icon";
import { GROUPS, type Route, TREE, groupOf, parentOf } from "./routes";
import "./settings-tree.css";

/** Where the window remembers which tree groups are expanded between launches (this window only, not `settings.json`). */
const STORAGE_KEY = "kinige.settingsTree.expanded";

function loadExpanded(): Route[] {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "[]");
    return Array.isArray(parsed) ? parsed.filter((item): item is Route => typeof item === "string" && item in GROUPS) : [];
  } catch {
    // There is no storage or the value is corrupted: all groups are collapsed.
    return [];
  }
}

function saveExpanded(groups: ReadonlySet<Route>) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify([...groups]));
  } catch {
    // The storage is unavailable: the state lives until the window closes.
  }
}

interface SettingsTreeProps {
  route: Route;
  onNavigate: (route: Route) => void;
  /** Items that have an error: a mark next to the name (for now this is "Файл настроек"). For a collapsed group, on the section. */
  withProblems?: readonly Route[];
  /** Groups expanded on open (Storybook stories); without it the saved state is used. */
  defaultExpanded?: readonly Route[];
  /** Routes that are not offered at all (Windows has no AI-agent access yet: no "Visible chats" and "Access log"). */
  without?: readonly Route[];
}

/**
 * The section tree of the "Settings" window: a `nav` with a list of buttons. The selected item is `aria-current="page"` and a fill (no outline);
 * nested items ("Видимые чаты", "Журнал обращений") stand under "Доступ ИИ-агентов" and collapse: the chevron on the left
 * collapses and expands the group, the name opens the section and expands the group. By default groups are collapsed, except
 * the one with the current page (it expands by itself on any navigation); a collapsed group with the current subitem is highlighted.
 * The state is remembered for the lifetime of the window and in `localStorage`.
 *
 * The keyboard follows the WAI-ARIA tree pattern: Up and Down move over visible items, Right expands a group (on an expanded one it moves to the first
 * nested item), Left collapses (on a nested item it moves to the section), Home and End go to the edge items, Enter and Space open the
 * page. `aria-expanded` is on a section with nested items. In the Tab order there is one item, the selected one, or if it is hidden in a
 * collapsed group, the section of that group. The focus ring is `:focus-visible` only (from the keyboard). There is no motion.
 */
export function SettingsTree({ route, onNavigate, withProblems = [], defaultExpanded, without = [] }: SettingsTreeProps) {
  const { t } = useTranslation();
  // The nested items that are offered; a group left without any is an ordinary item.
  const childrenOf = (item: Route): readonly Route[] | undefined => {
    const offered = GROUPS[item]?.filter((child) => !without.includes(child));
    return offered && offered.length > 0 ? offered : undefined;
  };
  const list = useRef<HTMLUListElement>(null);
  const [expanded, setExpanded] = useState<ReadonlySet<Route>>(() => {
    const open = new Set<Route>(defaultExpanded ?? loadExpanded());
    const current = groupOf(route);
    if (current) open.add(current);
    return open;
  });

  // Any navigation (the tree, "Настроить ›", "back", a file error) expands the group of the current page. The state
  // is adjusted during rendering, not in an effect: no extra frame with a collapsed group.
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
    const children = childrenOf(at);
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
  // The only item in the Tab order: the selected one, or if it is hidden in a collapsed group, the section of that group.
  const tabStop = hidden(route) ? parentOf(route) : route;

  return (
    <nav className="settings-tree" aria-label={t("prefs.treeLabel")}>
      <ul className="settings-tree__list" ref={list} onKeyDown={onKeyDown}>
        {TREE.filter((item) => !without.includes(item.route) && !hidden(item.route)).map((item) => {
          const children = childrenOf(item.route);
          const open = expanded.has(item.route);
          const current = item.route === route;
          // A collapsed group with the current subitem is highlighted; the subitem's error mark is visible on the section while the group is collapsed.
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
                  onClick={() => {
                    // The name both opens the section and expands the group, including when the page is already open (the route does not change).
                    if (children) toggle(item.route, true);
                    onNavigate(item.route);
                  }}
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
