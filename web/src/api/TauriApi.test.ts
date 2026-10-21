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
    expect(invoke).toHaveBeenCalledWith("list_messages", { chatId: "chat-1", before: null, limit: null });

    invoke.mockResolvedValue({ id: "m" });
    await new TauriApi().sendMessage("chat-1", "текст");
    expect(invoke).toHaveBeenCalledWith("send_message", { chatId: "chat-1", body: "текст" });

    invoke.mockResolvedValue(undefined);
    await new TauriApi().setTextSize("large");
    expect(invoke).toHaveBeenCalledWith("set_text_size", { size: "large" });
    await new TauriApi().openExternalUrl("https://example.com");
    expect(invoke).toHaveBeenCalledWith("open_external_url", { url: "https://example.com" });
  });

  it("ошибка Rust с кодом превращается в KinigeApiError с этим кодом", async () => {
    invoke.mockImplementation(() => Promise.reject({ code: "store_write_failed" }));
    const error = await new TauriApi().sendMessage("c", "x").catch((e: unknown) => e);
    expect(error).toBeInstanceOf(KinigeApiError);
    expect((error as KinigeApiError).code).toBe("store_write_failed");
  });

  it("всё чужое (отказ ACL строкой, неизвестный код, не объект) — internal", async () => {
    for (const reason of ["Command send_message not allowed by ACL", { code: "unknown_code" }, null, undefined, 42]) {
      invoke.mockImplementation(() => Promise.reject(reason));
      const error = await new TauriApi().listChats().catch((e: unknown) => e);
      expect((error as KinigeApiError).code).toBe("internal");
    }
  });
});
