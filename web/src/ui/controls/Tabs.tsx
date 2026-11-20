import { useRef, type KeyboardEvent } from "react";
import "./tabs.css";

export interface TabOption<T extends string> {
  value: T;
  label: string;
}

interface TabsProps<T extends string> {
  tabs: readonly TabOption<T>[];
  value: T;
  onChange: (value: T) => void;
  /** The name of the tab list for the screen reader. */
  label: string;
  /** The id prefix of the tabs and their panels: tab `x` is `${idPrefix}-tab-x` and controls `${idPrefix}-panel-x` (`tabPanelProps` gives the panel its side). */
  idPrefix: string;
}

/** The `id` and ARIA attributes of the panel that belongs to tab `value` (a `role="tabpanel"` labelled by its tab). */
export function tabPanelProps(idPrefix: string, value: string) {
  return { role: "tabpanel", id: `${idPrefix}-panel-${value}`, "aria-labelledby": `${idPrefix}-tab-${value}` } as const;
}

/**
 * A row of tabs (`tablist`): equal-width text buttons with a line under the selected one, which is also in the primary text, so the choice is not
 * told by colour alone. One tab is in the Tab order (the selected one, roving tabindex); the arrows, Home and End move to a tab and select it at
 * once (automatic activation), as in Telegram's panel. The caller draws the panel.
 */
export function Tabs<T extends string>({ tabs, value, onChange, label, idPrefix }: TabsProps<T>) {
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);

  const select = (index: number) => {
    const target = tabs[(index + tabs.length) % tabs.length];
    if (!target) return;
    buttons.current[tabs.indexOf(target)]?.focus();
    onChange(target.value);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const current = tabs.findIndex((tab) => tab.value === value);
    const steps: Record<string, number | undefined> = { ArrowRight: current + 1, ArrowLeft: current - 1, Home: 0, End: tabs.length - 1 };
    const to = steps[event.key];
    if (to === undefined) return;
    event.preventDefault();
    select(to);
  };

  return (
    <div className="tabs" role="tablist" aria-label={label} aria-orientation="horizontal" onKeyDown={onKeyDown}>
      {tabs.map((tab, index) => (
        <button
          key={tab.value}
          type="button"
          role="tab"
          id={`${idPrefix}-tab-${tab.value}`}
          aria-controls={`${idPrefix}-panel-${tab.value}`}
          aria-selected={tab.value === value}
          className="tabs__tab"
          tabIndex={tab.value === value ? 0 : -1}
          ref={(element) => {
            buttons.current[index] = element;
          }}
          onClick={() => onChange(tab.value)}
        >
          {tab.label}
        </button>
      ))}
    </div>
  );
}
