import { describe, expect, it } from "vitest";
import type { BackupStatus } from "../../api/generated/BackupStatus";
import { render, screen } from "../../test/render";
import { BackupRow } from "./BackupRow";
import { BACKUP_FOLDER, BACKUP_NOW, DEFAULT_BACKUP } from "./demoBackup";

const show = (patch: Partial<BackupStatus> = {}, language: "ru" | "en" = "ru", now: Date = BACKUP_NOW) =>
  render(<BackupRow backup={{ ...DEFAULT_BACKUP, ...patch }} now={now} />, { language });

describe("BackupRow", () => {
  it("«Копия базы: сегодня, 09:12», число копий и папка", () => {
    show();
    expect(screen.getByText("Копия базы: сегодня, 09:12")).toBeInTheDocument();
    expect(screen.getByText("7 копий")).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Папка копий" })).toHaveTextContent(BACKUP_FOLDER);
    expect(screen.getByTitle(BACKUP_FOLDER)).toBeInTheDocument();
    expect(screen.queryByRole("alert")).toBeNull();
    // Папку выбрать нельзя: кнопки нет.
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("вчера и более ранняя дата: метка дня как в ленте", () => {
    show({ lastAt: new Date(2026, 9, 26, 23, 40).getTime() });
    expect(screen.getByText("Копия базы: вчера, 23:40")).toBeInTheDocument();
  });

  it("копия давнее вчерашней: «14 марта, 08:05» и с годом для другого года", () => {
    const { unmount } = show({ lastAt: new Date(2026, 2, 14, 8, 5).getTime() });
    expect(screen.getByText("Копия базы: 14 марта, 08:05")).toBeInTheDocument();
    unmount();
    show({ lastAt: new Date(2025, 2, 14, 8, 5).getTime() });
    expect(screen.getByText("Копия базы: 14 марта 2025, 08:05")).toBeInTheDocument();
  });

  it("копий ещё нет: «ещё не снималась», числа нет", () => {
    show({ lastAt: null, count: 0 });
    expect(screen.getByText("Копия базы: ещё не снималась")).toBeInTheDocument();
    expect(screen.queryByText(/^\d+ копи(я|и|й)$/)).toBeNull();
  });

  it.each([
    [1, "1 копия"],
    [2, "2 копии"],
    [5, "5 копий"],
    [7, "7 копий"],
    [21, "21 копия"],
  ])("число копий (ru): %i", (count, text) => {
    show({ count });
    expect(screen.getByText(text)).toBeInTheDocument();
  });

  it("английский: метка, число, папка", () => {
    show({ count: 1 }, "en");
    expect(screen.getByText("Database backup: today, 09:12")).toBeInTheDocument();
    expect(screen.getByText("1 backup")).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Backup folder" })).toBeInTheDocument();
  });

  it("английский: вчера и дата", () => {
    const { unmount } = show({ lastAt: new Date(2026, 9, 26, 7, 5).getTime() }, "en");
    expect(screen.getByText("Database backup: yesterday, 07:05")).toBeInTheDocument();
    unmount();
    show({ lastAt: new Date(2026, 2, 14, 7, 5).getTime(), count: 3 }, "en");
    expect(screen.getByText("Database backup: March 14, 07:05")).toBeInTheDocument();
    expect(screen.getByText("3 backups")).toBeInTheDocument();
  });

  it("без папки строки папки нет", () => {
    show({ folder: null });
    expect(screen.queryByRole("group")).toBeNull();
  });

  it.each([
    ["folder_unavailable", /Папка копий недоступна/],
    ["write_failed", /Не удалось записать копию/],
  ] as const)("ошибка %s: role=alert без рамки-предупреждения", (error, text) => {
    const { container } = show({ error });
    expect(screen.getByRole("alert")).toHaveTextContent(text);
    expect(container.querySelector(".backup-row__warning")).toBeNull();
  });

  it("integrity_failed — заметное предупреждение (рамка цвета ошибки) и role=alert", () => {
    const { container } = show({ error: "integrity_failed" });
    const alert = screen.getByRole("alert");
    expect(alert).toHaveTextContent("Проверка нашла повреждение базы. Копия дня не снимается");
    expect(container.querySelector(".backup-row__warning")).toContainElement(alert);
  });

  it("английский: предупреждение о повреждении", () => {
    show({ error: "integrity_failed" }, "en");
    expect(screen.getByRole("alert")).toHaveTextContent("A check found damage in the database.");
  });

  it("без now берутся часы окна: не падает", () => {
    render(<BackupRow backup={DEFAULT_BACKUP} />);
    expect(screen.getByText(/^Копия базы: /)).toBeInTheDocument();
  });
});

describe("BackupRow: purge_failed", () => {
  it("автоудаление корзины не сработало: обычная строка ошибки, role=alert, без рамки-предупреждения", () => {
    const { container } = show({ error: "purge_failed" });
    expect(screen.getByRole("alert")).toHaveTextContent("Автоудаление корзины не смогло убрать повреждённую запись; она осталась в корзине.");
    expect(container.querySelector(".backup-row__warning")).toBeNull();
  });

  it("английский", () => {
    show({ error: "purge_failed" }, "en");
    expect(screen.getByRole("alert")).toHaveTextContent("Automatic trash cleanup could not remove a damaged entry");
  });
});
