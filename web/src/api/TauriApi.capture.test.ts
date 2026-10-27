import { beforeEach, describe, expect, it, vi } from "vitest";
import { KinigeApiError } from "./KinigeApi";

const invoke = vi.fn();
const listen = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: (...args: unknown[]) => listen(...args) }));

const { TauriApi } = await import("./TauriApi");

describe("TauriApi: главное окно (слайс 2)", () => {
  beforeEach(() => {
    invoke.mockReset();
    listen.mockReset();
  });

  it("черновик и сочетания: имена команд и аргументы", async () => {
    invoke.mockResolvedValue("");
    await new TauriApi().getDraft("chat-1");
    expect(invoke).toHaveBeenLastCalledWith("get_draft", { chatId: "chat-1" });
    await new TauriApi().saveDraft("chat-1", "текст");
    expect(invoke).toHaveBeenLastCalledWith("save_draft", { chatId: "chat-1", body: "текст" });
    await new TauriApi().setShortcut("capture", "Ctrl+Alt+KeyK");
    expect(invoke).toHaveBeenLastCalledWith("set_shortcut", { kind: "capture", accelerator: "Ctrl+Alt+KeyK" });
    await new TauriApi().resetShortcut("clipboard");
    expect(invoke).toHaveBeenLastCalledWith("reset_shortcut", { kind: "clipboard" });
    await new TauriApi().shortcutRecording(true);
    expect(invoke).toHaveBeenLastCalledWith("shortcut_recording", { active: true });
  });

  it("подписка на «записи изменились»: событие Rust entries-changed, возвращается отписка", async () => {
    const unlisten = vi.fn();
    listen.mockResolvedValue(unlisten);
    const handler = vi.fn();
    const off = await new TauriApi().onEntriesChanged(handler);
    expect(listen).toHaveBeenCalledWith("entries-changed", expect.any(Function));
    const callback = listen.mock.calls[0]?.[1] as (event: unknown) => void;
    callback({ payload: null });
    expect(handler).toHaveBeenCalledTimes(1);
    off();
    expect(unlisten).toHaveBeenCalled();
  });

  it("коды ошибок сочетаний приходят как KinigeApiError с этим кодом", async () => {
    for (const code of ["shortcut_invalid", "shortcut_taken", "shortcut_duplicate"]) {
      invoke.mockImplementation(() => Promise.reject({ code }));
      const error = await new TauriApi().setShortcut("capture", "x").catch((e: unknown) => e);
      expect(error).toBeInstanceOf(KinigeApiError);
      expect((error as KinigeApiError).code).toBe(code);
    }
  });
});

describe("TauriApi: окно захвата (слайс 2)", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("команды окна захвата и их аргументы: без chat_id", async () => {
    invoke.mockResolvedValue(undefined);
    const api = new TauriApi();
    await api.captureState();
    expect(invoke).toHaveBeenLastCalledWith("capture_state", undefined);
    await api.captureSend("текст");
    expect(invoke).toHaveBeenLastCalledWith("capture_send", { body: "текст" });
    await api.captureAppend("ещё");
    expect(invoke).toHaveBeenLastCalledWith("capture_append", { body: "ещё" });
    await api.captureSaveDraft("черновик");
    expect(invoke).toHaveBeenLastCalledWith("capture_save_draft", { body: "черновик" });
    await api.captureHide();
    expect(invoke).toHaveBeenLastCalledWith("capture_hide", undefined);
    const height = 172;
    await api.captureResize(height);
    expect(invoke).toHaveBeenLastCalledWith("capture_resize", { height });
    await api.captureAddRecentEmoji("📚");
    expect(invoke).toHaveBeenLastCalledWith("capture_add_recent_emoji", { emoji: "📚" });
    const names = invoke.mock.calls.map((call) => call[0]);
    expect(names.every((name: string) => name.startsWith("capture_"))).toBe(true);
    for (const call of invoke.mock.calls) {
      expect(JSON.stringify(call[1] ?? {})).not.toContain("chatId");
    }
  });
});
