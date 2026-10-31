import { describe, expect, it, vi } from "vitest";
import { MOCK_SETTINGS_FILE } from "../../../api/MockApiSlice4b";
import type { SettingsFileState } from "../../../api/generated/SettingsFileState";
import { fireEvent, render, screen, waitFor, within } from "../../../test/render";
import { FilePage } from "./FilePage";

const file = (patch: Partial<SettingsFileState> = {}): SettingsFileState => ({ ...MOCK_SETTINGS_FILE, ...patch });

describe("Файл настроек", () => {
  it("файл без ошибок: путь, «Копировать путь», тихая строка «Файл без ошибок»", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    render(<FilePage file={file()} />);
    expect(screen.getByRole("heading", { level: 2, name: "Файл настроек" })).toBeInTheDocument();
    expect(screen.getByTitle(MOCK_SETTINGS_FILE.path)).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("Файл без ошибок");
    expect(screen.queryByRole("alert")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Копировать путь" }));
    await waitFor(() => expect(writeText).toHaveBeenCalledWith(MOCK_SETTINGS_FILE.path));
    expect(await screen.findByRole("button", { name: "Скопировано" })).toBeInTheDocument();
  });

  it("буфер недоступен: честное «Не удалось скопировать»", async () => {
    Object.defineProperty(navigator, "clipboard", { value: { writeText: vi.fn().mockRejectedValue(new Error("нет")) }, configurable: true });
    render(<FilePage file={file()} />);
    fireEvent.click(screen.getByRole("button", { name: "Копировать путь" }));
    expect(await screen.findByRole("button", { name: "Не удалось скопировать" })).toBeInTheDocument();
  });

  it("файл не разобран: ошибка с позицией, «Файл без ошибок» нет", () => {
    render(<FilePage file={file({ problem: { kind: "parse", line: 1, column: 15 } })} />);
    expect(screen.getByRole("alert")).toHaveTextContent("Файл не разобран: строка 1, столбец 15. Ничего не изменено.");
    expect(screen.queryByText("Файл без ошибок")).toBeNull();
  });

  it.each([
    ["not_object", /объектом JSON/],
    ["not_utf8", /не в кодировке UTF-8/],
    ["bom", /метки порядка байтов/],
    ["too_large", /больше 1 МиБ/],
    ["io", /не смог прочитать или записать/],
  ] as const)("проблема файла «%s» называется по-русски", (kind, text) => {
    render(<FilePage file={file({ problem: { kind, line: null, column: null } })} />);
    expect(screen.getByRole("alert")).toHaveTextContent(text);
  });

  it.each([
    ["symlink", /символическая ссылка/],
    ["not_regular", /не обычный файл/],
    ["owner", /другому пользователю/],
    ["permissions", /chmod 600/],
    ["directory", /папку с данными/],
  ] as const)("небезопасный файл «%s»: причина, по которой ключи доступа и папки не применяются", (reason, text) => {
    render(<FilePage file={file({ unsafeReason: reason })} />);
    expect(screen.getByRole("alert")).toHaveTextContent(text);
    expect(screen.queryByText("Файл без ошибок")).toBeNull();
  });

  it("ошибки ключей: строка на ключ с его именем; неизвестные ключи — предупреждение", () => {
    render(
      <FilePage
        file={file({
          issues: [
            { key: "text_size", kind: "value" },
            { key: "agent_access.chats[2]", kind: "chat_not_found" },
            { key: "shortcuts.capture", kind: "shortcut_taken" },
            { key: "mirror.folder", kind: "folder_inside_data" },
          ],
          unknownKeys: ["surprise", "shortcuts.extra"],
        })}
      />,
    );
    const problems = screen.getByRole("list", { name: "Проблемы" });
    expect(within(problems).getAllByRole("listitem")).toHaveLength(4);
    expect(problems).toHaveTextContent("text_size: недопустимое значение");
    expect(problems).toHaveTextContent("agent_access.chats[2]: такого чата нет");
    expect(problems).toHaveTextContent("shortcuts.capture: сочетание занято");
    expect(problems).toHaveTextContent("mirror.folder: папка не должна лежать внутри каталога данных");
    const unknown = screen.getByRole("list", { name: "Неизвестные ключи" });
    expect(unknown).toHaveTextContent("surprise: Kinige не знает этот ключ");
    expect(screen.queryByText("Файл без ошибок")).toBeNull();
  });

  it("неизвестный ключ — предупреждение, а не ошибка файла: «Файл без ошибок» остаётся", () => {
    render(<FilePage file={file({ unknownKeys: ["surprise"] })} />);
    expect(screen.getByRole("status")).toHaveTextContent("Файл без ошибок");
  });

  it("длинный путь переносится, а не вылезает: путь целиком в разметке", () => {
    const path = `/Users/me/Library/Application Support/${"очень-длинная-папка/".repeat(8)}settings.json`;
    render(<FilePage file={file({ path })} />);
    expect(screen.getByTitle(path).textContent).toBe(path);
  });

  it("английский", () => {
    render(<FilePage file={file({ problem: { kind: "parse", line: 3, column: 9 } })} />, { language: "en" });
    expect(screen.getByRole("alert")).toHaveTextContent("The file could not be read: line 3, column 9. Nothing was changed.");
    expect(screen.getByRole("button", { name: "Copy path" })).toBeInTheDocument();
  });
});
