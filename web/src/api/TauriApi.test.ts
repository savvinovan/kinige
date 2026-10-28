import { beforeEach, describe, expect, it, vi } from "vitest";
import { KinigeApiError } from "./KinigeApi";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

const { TauriApi } = await import("./TauriApi");

describe("TauriApi", () => {
  beforeEach(() => {
    // Без фигурных скобок хук вернул бы мок, а vitest вызвал бы его как «очистку» после теста.
    invoke.mockReset();
  });

  it("команды и аргументы: camelCase, курсор и лимит по умолчанию — null", async () => {
    invoke.mockResolvedValue({ messages: [], older: null, today: "2026-10-21" });
    await new TauriApi().listMessages("chat-1");
    expect(invoke).toHaveBeenCalledWith("list_messages", {
      chatId: "chat-1",
      before: null,
      after: null,
      around: null,
      limit: null,
      tag: null,
    });

    invoke.mockResolvedValue({ id: "m" });
    await new TauriApi().sendMessage("chat-1", "текст");
    expect(invoke).toHaveBeenCalledWith("send_message", { chatId: "chat-1", body: "текст" });

    invoke.mockResolvedValue(undefined);
    await new TauriApi().setTextSize("large");
    expect(invoke).toHaveBeenCalledWith("set_text_size", { size: "large" });
    await new TauriApi().openExternalUrl("https://example.com");
    expect(invoke).toHaveBeenCalledWith("open_external_url", { url: "https://example.com" });
  });

  it("импорт и поиск: команды слайса 4, аргументы camelCase, путь к файлу из JS не передаётся", async () => {
    invoke.mockResolvedValue(null);
    await new TauriApi().importTelegramOpen();
    // Файл выбирает диалог в Rust: аргументов у команды нет.
    expect(invoke).toHaveBeenLastCalledWith("import_telegram_open", undefined);

    invoke.mockResolvedValue({ imported: 1 });
    await new TauriApi().importTelegramRun(7, ["1", "-100"]);
    expect(invoke).toHaveBeenLastCalledWith("import_telegram_run", { session: 7, chatIds: ["1", "-100"] });

    invoke.mockResolvedValue(undefined);
    await new TauriApi().importTelegramStop();
    expect(invoke).toHaveBeenLastCalledWith("import_telegram_stop", undefined);
    await new TauriApi().importTelegramClose();
    expect(invoke).toHaveBeenLastCalledWith("import_telegram_close", undefined);

    invoke.mockResolvedValue({ chats: [], hits: [], older: null });
    await new TauriApi().searchMessages("книги");
    expect(invoke).toHaveBeenLastCalledWith("search_messages", { query: "книги", filters: null, before: null, limit: null });
    const filters = { chatId: "c", favorite: true, tasks: false, from: "2026-01-01", to: null };
    const before = { journalDate: "2026-10-01", createdAt: 5, id: "m" };
    await new TauriApi().searchMessages("книги", { filters, before, limit: 20 });
    expect(invoke).toHaveBeenLastCalledWith("search_messages", { query: "книги", filters, before, limit: 20 });

    await new TauriApi().listMessages("chat-1", { around: "m-1", limit: 50 });
    expect(invoke).toHaveBeenLastCalledWith("list_messages", {
      chatId: "chat-1",
      before: null,
      after: null,
      around: "m-1",
      limit: 50,
      tag: null,
    });
  });

  it("ошибка Rust с кодом превращается в KinigeApiError с этим кодом", async () => {
    invoke.mockImplementation(() => Promise.reject({ code: "store_write_failed" }));
    const error = await new TauriApi().sendMessage("c", "x").catch((e: unknown) => e);
    expect(error).toBeInstanceOf(KinigeApiError);
    expect((error as KinigeApiError).code).toBe("store_write_failed");
  });

  it("доступ ИИ-агентов: команды слайса 4a и аргументы camelCase", async () => {
    invoke.mockResolvedValue({ enabled: false });
    await new TauriApi().getAgentAccess();
    expect(invoke).toHaveBeenLastCalledWith("get_agent_access", undefined);
    invoke.mockResolvedValue(undefined);
    await new TauriApi().setAgentEnabled(true);
    expect(invoke).toHaveBeenLastCalledWith("set_agent_enabled", { enabled: true });
    await new TauriApi().setAgentMode("read_write");
    expect(invoke).toHaveBeenLastCalledWith("set_agent_mode", { mode: "read_write" });
    await new TauriApi().setAgentChats(["a", "b"]);
    expect(invoke).toHaveBeenLastCalledWith("set_agent_chats", { chatIds: ["a", "b"] });
    invoke.mockResolvedValue({ rows: [], older: null });
    await new TauriApi().listAgentLog();
    expect(invoke).toHaveBeenLastCalledWith("list_agent_log", { before: null });
    await new TauriApi().listAgentLog(41);
    expect(invoke).toHaveBeenLastCalledWith("list_agent_log", { before: 41 });
    invoke.mockResolvedValue([]);
    await new TauriApi().listAgentLogEntries(7);
    expect(invoke).toHaveBeenLastCalledWith("list_agent_log_entries", { logId: 7 });
  });

  it("всё чужое (отказ ACL строкой, неизвестный код, не объект) — internal", async () => {
    for (const reason of ["Command send_message not allowed by ACL", { code: "unknown_code" }, null, undefined, 42]) {
      invoke.mockImplementation(() => Promise.reject(reason));
      const error = await new TauriApi().listChats().catch((e: unknown) => e);
      expect((error as KinigeApiError).code).toBe("internal");
    }
  });
});
