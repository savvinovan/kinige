import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, within } from "../../test/render";
import { SettingsTree } from "./SettingsTree";
import type { Route } from "./routes";

function Host({ start = "appearance", problems = [] }: { start?: Route; problems?: Route[] }) {
  const [route, setRoute] = useState<Route>(start);
  return <SettingsTree route={route} onNavigate={setRoute} withProblems={problems} />;
}

const nav = () => screen.getByRole("navigation", { name: "Разделы настроек" });
const names = () =>
  within(nav())
    .getAllByRole("button")
    .map((button) => button.textContent);
const access = () => within(nav()).getByRole("button", { name: /^Доступ ИИ-агентов/ });
const chevron = () => screen.getByTestId("tree-chevron-access");
const press = (target: HTMLElement, key: string) => fireEvent.keyDown(target, { key });

describe("Дерево настроек: сворачивание", () => {
  it("по умолчанию группа свёрнута: вложенных пунктов нет, у раздела aria-expanded=false", () => {
    render(<Host />);
    expect(names()).not.toContain("Видимые чаты");
    expect(access()).toHaveAttribute("aria-expanded", "false");
    // У пунктов без вложенных aria-expanded нет.
    expect(within(nav()).getByRole("button", { name: "Копии" })).not.toHaveAttribute("aria-expanded");
  });

  it("щелчок по шеврону разворачивает и сворачивает группу, страница при этом не меняется", () => {
    const { container } = render(<Host />);
    fireEvent.click(chevron());
    expect(names()).toContain("Видимые чаты");
    expect(access()).toHaveAttribute("aria-expanded", "true");
    expect(within(nav()).getByRole("button", { current: "page" })).toHaveTextContent("Оформление");
    fireEvent.click(chevron());
    expect(names()).not.toContain("Видимые чаты");
    expect(container.querySelector(".settings-tree__row[data-current]")).toHaveTextContent("Оформление");
  });

  it("щелчок по названию открывает раздел и разворачивает группу", () => {
    render(<Host />);
    fireEvent.click(access());
    expect(within(nav()).getByRole("button", { current: "page" })).toHaveTextContent("Доступ ИИ-агентов");
    expect(names()).toContain("Журнал обращений");
    expect(access()).toHaveAttribute("aria-expanded", "true");
  });

  it("страница внутри группы при открытии: группа развёрнута сама, выбранный подпункт отмечен", () => {
    render(<Host start="access/log" />);
    expect(names()).toContain("Журнал обращений");
    expect(within(nav()).getByRole("button", { current: "page" })).toHaveTextContent("Журнал обращений");
  });

  it("переход на подэкран (например, «Настроить ›») разворачивает свёрнутую группу", () => {
    const { rerender } = render(<SettingsTree route="appearance" onNavigate={() => undefined} />);
    expect(names()).not.toContain("Видимые чаты");
    rerender(<SettingsTree route="access/chats" onNavigate={() => undefined} />);
    expect(names()).toContain("Видимые чаты");
    expect(within(nav()).getByRole("button", { current: "page" })).toHaveTextContent("Видимые чаты");
  });

  it("свёрнутая группа с текущим подпунктом подсвечивает раздел, а aria-current остаётся у подпункта", () => {
    const { container } = render(<Host start="access/chats" />);
    fireEvent.click(chevron());
    expect(names()).not.toContain("Видимые чаты");
    expect(container.querySelector(".settings-tree__row[data-contains-current]")).toHaveTextContent("Доступ ИИ-агентов");
    expect(within(nav()).queryByRole("button", { current: "page" })).toBeNull();
  });

  it("метка ошибки подпункта видна на разделе, пока группа свёрнута, и на самом подпункте, когда развёрнута", () => {
    render(<Host problems={["access/log"]} />);
    expect(within(nav()).getByRole("button", { name: "Доступ ИИ-агентов, есть проблемы" })).toBeInTheDocument();
    fireEvent.click(chevron());
    expect(within(nav()).getByRole("button", { name: "Доступ ИИ-агентов" })).toBeInTheDocument();
    expect(within(nav()).getByRole("button", { name: "Журнал обращений, есть проблемы" })).toBeInTheDocument();
  });

  it("метка ошибки у раздела без вложенных остаётся как была", () => {
    render(<Host problems={["file"]} />);
    expect(within(nav()).getByRole("button", { name: "Файл настроек, есть проблемы" })).toBeInTheDocument();
  });

  it("выбранный пункт — только заливка: в стилях дерева нет рамки, контура и тени", () => {
    const css = readFileSync(resolve(__dirname, "settings-tree.css"), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
    expect(css).not.toMatch(/box-shadow\s*:/);
    expect(css).not.toMatch(/(^|[;{\s])outline\s*:/);
    // Единственная линия — разделитель правого края колонки (`border-right` у `.settings-tree`), не обводка пункта.
    const borders = [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)]
      .filter(([, , body]) => /\bborder(?:-(?:top|right|bottom|left|width|style|color))*\s*:(?!\s*0\s*;)[^;]+;/.test(body ?? ""))
      .map((match) => match[1]?.trim());
    expect(borders).toEqual([".settings-tree"]);
  });
});

describe("Дерево настроек: клавиатура по образцу WAI-ARIA tree", () => {
  it("→ разворачивает свёрнутую группу, → на развёрнутой переходит к первому вложенному, ← возвращает", () => {
    render(<Host />);
    access().focus();
    press(access(), "ArrowRight");
    expect(access()).toHaveAttribute("aria-expanded", "true");
    expect(access()).toHaveFocus();
    press(access(), "ArrowRight");
    const first = within(nav()).getByRole("button", { name: "Видимые чаты" });
    expect(first).toHaveFocus();
    press(first, "ArrowLeft");
    expect(access()).toHaveFocus();
    press(access(), "ArrowLeft");
    expect(access()).toHaveAttribute("aria-expanded", "false");
    expect(names()).not.toContain("Видимые чаты");
  });

  it("↑ и ↓ ходят только по видимым пунктам: свёрнутую группу ↓ перепрыгивает", () => {
    render(<Host />);
    access().focus();
    press(access(), "ArrowDown");
    expect(within(nav()).getByRole("button", { name: "Файл настроек" })).toHaveFocus();
    press(within(nav()).getByRole("button", { name: "Файл настроек" }), "ArrowUp");
    expect(access()).toHaveFocus();
    press(access(), "ArrowRight");
    press(access(), "ArrowDown");
    expect(within(nav()).getByRole("button", { name: "Видимые чаты" })).toHaveFocus();
  });

  it("Home и End — крайние видимые пункты; ← на разделе без вложенных ничего не ломает", () => {
    render(<Host />);
    const copies = within(nav()).getByRole("button", { name: "Копии" });
    copies.focus();
    press(copies, "ArrowLeft");
    press(copies, "ArrowRight");
    expect(copies).toHaveFocus();
    press(copies, "End");
    expect(within(nav()).getByRole("button", { name: "Файл настроек" })).toHaveFocus();
    press(within(nav()).getByRole("button", { name: "Файл настроек" }), "Home");
    expect(within(nav()).getByRole("button", { name: "Оформление" })).toHaveFocus();
  });

  it("Enter на пункте — обычное нажатие кнопки: открывает раздел и разворачивает группу", () => {
    render(<Host />);
    fireEvent.click(access());
    expect(access()).toHaveAttribute("aria-current", "page");
    expect(names()).toContain("Видимые чаты");
  });

  it("в обходе Tab один пункт: выбранный, а если он скрыт в свёрнутой группе — её раздел", () => {
    render(<Host start="access/log" />);
    const stops = () => within(nav()).getAllByRole("button").filter((button) => button.tabIndex === 0);
    expect(stops()).toHaveLength(1);
    expect(stops()[0]).toHaveTextContent("Журнал обращений");
    fireEvent.click(chevron());
    expect(stops()).toHaveLength(1);
    expect(stops()[0]).toHaveTextContent("Доступ ИИ-агентов");
  });
});

describe("Дерево настроек: состояние помнится", () => {
  it("развёрнутая группа записывается в localStorage и восстанавливается при следующем открытии", () => {
    const first = render(<Host />);
    fireEvent.click(chevron());
    expect(JSON.parse(localStorage.getItem("kinige.settingsTree.expanded") ?? "[]")).toEqual(["access"]);
    first.unmount();
    render(<Host />);
    expect(names()).toContain("Видимые чаты");
  });

  it("повреждённое сохранённое значение и недоступное хранилище не ломают дерево", () => {
    localStorage.setItem("kinige.settingsTree.expanded", "{не json");
    const first = render(<Host />);
    expect(names()).not.toContain("Видимые чаты");
    first.unmount();
    localStorage.setItem("kinige.settingsTree.expanded", JSON.stringify(["access", "нет-такого", 5]));
    const second = render(<Host />);
    expect(names()).toContain("Видимые чаты");
    second.unmount();
    const spy = vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("нет хранилища");
    });
    const setSpy = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("нет хранилища");
    });
    render(<Host />);
    fireEvent.click(chevron());
    expect(names()).toContain("Видимые чаты");
    spy.mockRestore();
    setSpy.mockRestore();
  });

  it("истории: defaultExpanded важнее сохранённого и ничего не пишет в хранилище", () => {
    localStorage.setItem("kinige.settingsTree.expanded", JSON.stringify(["access"]));
    render(<SettingsTree route="appearance" onNavigate={() => undefined} defaultExpanded={[]} />);
    expect(names()).not.toContain("Видимые чаты");
    fireEvent.click(chevron());
    expect(localStorage.getItem("kinige.settingsTree.expanded")).toBe(JSON.stringify(["access"]));
  });
});
