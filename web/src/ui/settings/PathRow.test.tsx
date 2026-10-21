import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "../../test/render";
import { PathRow } from "./PathRow";

describe("PathRow", () => {
  it("подпись, путь, бейдж «Папка по умолчанию» и кнопка «Выбрать…»", () => {
    render(<PathRow path="~/Documents/Kinige" isDefault onChoose={() => {}} />);
    expect(screen.getByRole("group", { name: "Папка" })).toBeInTheDocument();
    expect(screen.getByText("~/Documents/Kinige")).toBeInTheDocument();
    expect(screen.getByText("Папка по умолчанию")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Выбрать…" })).toBeInTheDocument();
  });

  it("не по умолчанию: бейджа нет", () => {
    render(<PathRow path="/Users/me/Notes" onChoose={() => {}} />);
    expect(screen.queryByText("Папка по умолчанию")).toBeNull();
  });

  it("папка не выбрана: «Папка не выбрана», кнопка на месте", () => {
    render(<PathRow path={null} isDefault onChoose={() => {}} />);
    expect(screen.getByText("Папка не выбрана")).toBeInTheDocument();
    expect(screen.queryByText("Папка по умолчанию")).toBeNull();
    expect(screen.getByRole("button", { name: "Выбрать…" })).toBeInTheDocument();
  });

  it("длинный путь целиком в тексте и в title, кнопка описана путём", () => {
    const path = "/Users/Александр/Документы/Очень длинное название папки/Журнал для заметок/Kinige-копия-2026";
    render(<PathRow path={path} onChoose={() => {}} />);
    const code = screen.getByText(path);
    expect(code).toHaveAttribute("title", path);
    expect(code.textContent).toBe(path);
    expect(screen.getByRole("button", { name: "Выбрать…" })).toHaveAccessibleDescription(path);
  });

  it("«Выбрать…» вызывает onChoose", () => {
    const onChoose = vi.fn();
    render(<PathRow path={null} onChoose={onChoose} />);
    fireEvent.click(screen.getByRole("button", { name: "Выбрать…" }));
    expect(onChoose).toHaveBeenCalledTimes(1);
  });

  it("busy: aria-disabled, остаётся в обходе, onChoose не вызывается", () => {
    const onChoose = vi.fn();
    render(<PathRow path={null} busy onChoose={onChoose} />);
    const button = screen.getByRole("button", { name: "Выбрать…" });
    expect(button).toHaveAttribute("aria-disabled", "true");
    expect(button).toHaveAttribute("aria-busy", "true");
    expect(button).not.toBeDisabled();
    fireEvent.click(button);
    expect(onChoose).not.toHaveBeenCalled();
  });

  it("английский", () => {
    render(<PathRow path={null} onChoose={() => {}} />, { language: "en" });
    expect(screen.getByRole("group", { name: "Folder" })).toBeInTheDocument();
    expect(screen.getByText("No folder selected")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Choose…" })).toBeInTheDocument();
  });
});
