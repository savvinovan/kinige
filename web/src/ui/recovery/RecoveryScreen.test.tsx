import { fireEvent, render, screen, within } from "../../test/render";
import { describe, expect, it, vi } from "vitest";
import type { BackupInfo } from "../../api/generated/BackupInfo";
import { RecoveryScreen } from "./RecoveryScreen";

// Время копий — местное: подписи не зависят от пояса машины.
const backup = (name: string, kind: BackupInfo["kind"], at: Date): BackupInfo => ({ name, kind, createdAt: at.getTime() });
const OLD = backup("daily-2026-10-25.db", "daily", new Date(2026, 9, 25, 9, 12));
const MID = backup("pre-migration-2026-10-26.db", "pre_migration", new Date(2026, 9, 26, 18, 30));
const NEW = backup("daily-2026-10-27.db", "daily", new Date(2026, 9, 27, 8, 5));

type Props = Partial<React.ComponentProps<typeof RecoveryScreen>>;
const show = (props: Props = {}, language: "ru" | "en" = "ru") => {
  const onRestore = vi.fn();
  const onRestart = vi.fn();
  const view = render(
    <RecoveryScreen
      mode="startup"
      backups={[OLD, NEW, MID]}
      language={language}
      onRestore={onRestore}
      onRestart={onRestart}
      {...props}
    />,
    { language },
  );
  return { onRestore, onRestart, ...view };
};

describe("RecoveryScreen: startup", () => {
  it("заголовок, пояснение (файл не удаляется, запись после копии могла не сохраниться), экран — главная область", () => {
    show();
    expect(screen.getByRole("main", { name: "База повреждена" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1, name: "База повреждена" })).toBeInTheDocument();
    expect(screen.getByText(/не удаляется/)).toBeInTheDocument();
    expect(screen.getByText(/после последней копии/)).toBeInTheDocument();
  });

  it("копии новые первыми, подписаны датой, временем и видом; радиогруппа", () => {
    show();
    const group = screen.getByRole("radiogroup", { name: "Копии базы" });
    const rows = within(group).getAllByRole("radio");
    expect(rows.map((radio) => (radio as HTMLInputElement).value)).toEqual([NEW.name, MID.name, OLD.name]);
    const labels = [...group.querySelectorAll("label")].map((label) => label.textContent);
    expect(labels).toEqual(["27 октября 2026, 08:05копия дня", "26 октября 2026, 18:30перед обновлением", "25 октября 2026, 09:12копия дня"]);
  });

  it("по умолчанию выбрана самая новая; кнопка называет её дату и время", () => {
    const { onRestore } = show();
    expect((screen.getAllByRole("radio")[0] as HTMLInputElement).checked).toBe(true);
    const button = screen.getByRole("button", { name: "Восстановить из копии от 27 октября 2026, 08:05" });
    fireEvent.click(button);
    expect(onRestore).toHaveBeenCalledExactlyOnceWith(NEW.name);
  });

  it("выбор другой копии меняет кнопку и то, что восстанавливается", () => {
    const { onRestore } = show();
    fireEvent.click(screen.getAllByRole("radio")[2] as HTMLElement);
    const button = screen.getByRole("button", { name: "Восстановить из копии от 25 октября 2026, 09:12" });
    fireEvent.click(button);
    expect(onRestore).toHaveBeenCalledWith(OLD.name);
  });

  it("порядок во входных данных не важен", () => {
    show({ backups: [MID, OLD, NEW] });
    expect(screen.getByRole("button", { name: /27 октября 2026/ })).toBeInTheDocument();
  });

  it("фокус при открытии на выбранной (новой) копии", () => {
    show();
    expect(screen.getAllByRole("radio")[0]).toHaveFocus();
  });

  it("одна копия", () => {
    show({ backups: [OLD] });
    expect(screen.getAllByRole("radio")).toHaveLength(1);
    expect(screen.getByRole("button", { name: /25 октября 2026, 09:12/ })).toBeInTheDocument();
  });

  it("копий нет: «Копий нет», подсказка, кнопки восстановления нет", () => {
    show({ backups: [] });
    expect(screen.getByText("Копий нет")).toBeInTheDocument();
    expect(screen.getByText(/Сохраните файл базы/)).toBeInTheDocument();
    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.queryByRole("radiogroup")).toBeNull();
  });

  it("идёт восстановление (busy): кнопка недоступна и ничего не вызывает, выбор заблокирован", () => {
    const { onRestore } = show({ busy: true });
    const button = screen.getByRole("button", { name: "Восстанавливаем…" });
    expect(button).toHaveAttribute("aria-disabled", "true");
    expect(button).toHaveAttribute("aria-busy", "true");
    fireEvent.click(button);
    expect(onRestore).not.toHaveBeenCalled();
    for (const radio of screen.getAllByRole("radio")) expect(radio).toBeDisabled();
  });

  it("ошибка — role=alert, кнопка остаётся", () => {
    show({ error: "Копия не найдена или не открывается." });
    expect(screen.getByRole("alert")).toHaveTextContent("Копия не найдена или не открывается.");
    expect(screen.getByRole("button", { name: /Восстановить из копии/ })).toBeInTheDocument();
  });

  it("английский: дата и время, «daily backup», «before an update»", () => {
    show({ backups: [NEW, MID] }, "en");
    expect(screen.getByRole("heading", { name: "The database is damaged" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Restore from the backup of October 27, 2026, 08:05" })).toBeInTheDocument();
    expect(screen.getByText("daily backup")).toBeInTheDocument();
    expect(screen.getByText("before an update")).toBeInTheDocument();
  });

  it("список пропавшей выбранной копии: выбор возвращается на самую новую", () => {
    const { rerender } = show();
    fireEvent.click(screen.getAllByRole("radio")[2] as HTMLElement);
    rerender(<RecoveryScreen mode="startup" backups={[NEW, MID]} language="ru" onRestore={vi.fn()} onRestart={vi.fn()} />);
    expect(screen.getByRole("button", { name: /27 октября 2026, 08:05/ })).toBeInTheDocument();
  });
});

describe("RecoveryScreen: running", () => {
  it("заголовок, пояснение про перезапуск, кнопка «Перезапустить»; списка и «Восстановить» нет", () => {
    const { onRestart, onRestore } = show({ mode: "running" });
    expect(screen.getByRole("heading", { name: "База повреждена" })).toBeInTheDocument();
    expect(screen.getByText(/перезапустится и предложит копии/)).toBeInTheDocument();
    expect(screen.queryByRole("radiogroup")).toBeNull();
    expect(screen.queryByRole("button", { name: /Восстановить/ })).toBeNull();
    const restart = screen.getByRole("button", { name: "Перезапустить" });
    expect(restart).toHaveFocus();
    fireEvent.click(restart);
    expect(onRestart).toHaveBeenCalledTimes(1);
    expect(onRestore).not.toHaveBeenCalled();
  });

  it("идёт перезапуск (busy) и ошибка", () => {
    const { onRestart } = show({ mode: "running", busy: true, error: "Что-то пошло не так." });
    const restart = screen.getByRole("button", { name: "Перезапустить" });
    expect(restart).toHaveAttribute("aria-disabled", "true");
    fireEvent.click(restart);
    expect(onRestart).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent("Что-то пошло не так.");
  });

  it("английский", () => {
    show({ mode: "running" }, "en");
    expect(screen.getByRole("button", { name: "Restart" })).toBeInTheDocument();
  });
});

describe("RecoveryScreen: mirrorFolder", () => {
  it("startup: тихая строка под пояснением; без папки и в режиме running её нет", () => {
    const { unmount } = show({ mirrorFolder: "/Users/me/Documents/Kinige" });
    expect(
      screen.getByText("Папка Markdown-копии «/Users/me/Documents/Kinige» будет переименована в «…-before-restore-…» рядом: записи после снимка копии не потеряются."),
    ).toBeInTheDocument();
    unmount();
    const second = show({ mirrorFolder: null });
    expect(screen.queryByText(/Markdown-копии/)).toBeNull();
    second.unmount();
    show({ mode: "running", mirrorFolder: "/Users/me/Documents/Kinige" });
    expect(screen.queryByText(/Markdown-копии/)).toBeNull();
  });

  it("строка есть и когда копий нет (кнопки восстановления всё равно нет)", () => {
    show({ backups: [], mirrorFolder: "/x" });
    expect(screen.getByText(/Markdown-копии «\/x»/)).toBeInTheDocument();
  });

  it("английский", () => {
    show({ mirrorFolder: "/Users/me/Kinige" }, "en");
    expect(screen.getByText(/The Markdown copy folder “\/Users\/me\/Kinige” will be renamed/)).toBeInTheDocument();
  });
});
