import { fireEvent, render, screen } from "../../test/render";
import { describe, expect, it } from "vitest";
import { ProfileMenu } from "./ProfileMenu";

const open = () => {
  const button = screen.getByRole("button", { expanded: false });
  fireEvent.click(button);
  return button;
};
const items = () => screen.getAllByRole("menuitem");

describe("ProfileMenu", () => {
  it("по умолчанию «Я», аватар с инициалом скрыт от диктора, меню закрыто", () => {
    const { container } = render(<ProfileMenu />);
    const button = screen.getByRole("button", { name: "Профиль: Я" });
    expect(button).toHaveAttribute("aria-haspopup", "menu");
    expect(button).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("menu")).toBeNull();
    expect(container.querySelector(".profile__avatar")).toHaveAttribute("aria-hidden", "true");
  });

  it("инициалы из имени; длинное имя остаётся в тексте кнопки (обрезается стилем)", () => {
    const { container } = render(<ProfileMenu name="Александр Саввинов-Очень-Длинная-Фамилия" />);
    expect(container.querySelector(".profile__avatar")?.textContent).toBe("АС");
    expect(screen.getByRole("button")).toHaveAccessibleName("Профиль: Александр Саввинов-Очень-Длинная-Фамилия");
  });

  it("щелчок открывает меню «Настройки», «Экспорт», «О программе»; фокус на первом пункте", () => {
    render(<ProfileMenu />);
    const button = open();
    expect(button).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("menu", { name: "Профиль" })).toBeInTheDocument();
    expect(items().map((item) => item.textContent)).toEqual(["Настройки", "Экспорт", "О программе"]);
    expect(items()[0]).toHaveFocus();
  });

  it("пункты только для вида: aria-disabled, пояснение «Появится позже», нажатие ничего не делает", () => {
    render(<ProfileMenu />);
    open();
    for (const item of items()) {
      expect(item).toHaveAttribute("aria-disabled", "true");
      expect(item).toHaveAccessibleDescription("Появится позже");
    }
    fireEvent.click(items()[1] as HTMLElement);
    expect(screen.getByRole("menu")).toBeInTheDocument();
  });

  it("стрелки ходят по кругу, Home и End — к краям", () => {
    render(<ProfileMenu />);
    open();
    const [settings, exportItem, about] = items() as [HTMLElement, HTMLElement, HTMLElement];
    const menu = screen.getByRole("menu");
    fireEvent.keyDown(menu, { key: "ArrowDown" });
    expect(exportItem).toHaveFocus();
    fireEvent.keyDown(menu, { key: "ArrowDown" });
    expect(about).toHaveFocus();
    fireEvent.keyDown(menu, { key: "ArrowDown" });
    expect(settings).toHaveFocus();
    fireEvent.keyDown(menu, { key: "ArrowUp" });
    expect(about).toHaveFocus();
    fireEvent.keyDown(menu, { key: "Home" });
    expect(settings).toHaveFocus();
    fireEvent.keyDown(menu, { key: "End" });
    expect(about).toHaveFocus();
  });

  it("Esc закрывает меню и возвращает фокус на кнопку", () => {
    render(<ProfileMenu />);
    const button = open();
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });
    expect(screen.queryByRole("menu")).toBeNull();
    expect(button).toHaveAttribute("aria-expanded", "false");
    expect(button).toHaveFocus();
  });

  it("стрелка вниз на кнопке открывает меню с первого пункта, стрелка вверх — с последнего", () => {
    const { unmount } = render(<ProfileMenu />);
    const button = screen.getByRole("button");
    button.focus();
    fireEvent.keyDown(button, { key: "ArrowDown" });
    expect(items()[0]).toHaveFocus();
    unmount();

    render(<ProfileMenu />);
    const again = screen.getByRole("button");
    again.focus();
    fireEvent.keyDown(again, { key: "ArrowUp" });
    expect(items()[2]).toHaveFocus();
  });

  it("Tab и щелчок вне меню закрывают его", () => {
    render(<ProfileMenu />);
    open();
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Tab" });
    expect(screen.queryByRole("menu")).toBeNull();

    fireEvent.click(screen.getByRole("button"));
    expect(screen.getByRole("menu")).toBeInTheDocument();
    fireEvent.pointerDown(document.body);
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("повторный щелчок по кнопке закрывает меню", () => {
    render(<ProfileMenu />);
    const button = open();
    fireEvent.click(button);
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("точка статуса: в D1 не показывается, с пропом statusDot рисуется", () => {
    const { container, rerender } = render(<ProfileMenu />);
    expect(container.querySelector(".profile__dot")).toBeNull();
    rerender(<ProfileMenu statusDot />);
    expect(container.querySelector(".profile__dot")).not.toBeNull();
  });

  it("compact (свёрнутый сайдбар): только аватар, имя и меню остаются в подписи и в дереве", () => {
    const { container } = render(<ProfileMenu compact name="Александр" />);
    expect(container.querySelector(".profile")).toHaveClass("profile--compact");
    expect(container.querySelector(".profile__name")).toBeNull();
    expect(container.querySelector(".profile__chevron")).toBeNull();
    const button = screen.getByRole("button", { name: "Профиль: Александр" });
    fireEvent.click(button);
    expect(screen.getByRole("menu", { name: "Профиль" })).toBeInTheDocument();
    expect(items()).toHaveLength(3);
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });
    expect(button).toHaveFocus();
  });
});
