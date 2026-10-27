import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, MockApi } from "../../api/MockApi";
import type { StartupState } from "../../api/generated/StartupState";
import { render, screen } from "../../test/render";
import { AppRoot } from "./AppRoot";
import { RECOVERY_SETTINGS, loadStartup } from "./startupSettings";

const NORMAL: StartupState = { needsRecovery: false, backups: [], databaseTooNew: false, corruptionFound: false };

describe("loadStartup", () => {
  it("база открыта: настройки из Rust, состояние как есть", async () => {
    const api = new MockApi();
    const loaded = await loadStartup(api, NORMAL);
    expect(loaded.settings).toEqual(DEFAULT_SETTINGS);
    expect(loaded.startup).toEqual(NORMAL);
  });

  it("журнал не открыт или база новее приложения: настроек не читаем (их не из чего читать)", async () => {
    const api = new MockApi();
    api.failNext("getSettings", "internal");
    for (const state of [{ ...NORMAL, needsRecovery: true }, { ...NORMAL, databaseTooNew: true }]) {
      const loaded = await loadStartup(api, state);
      expect(loaded.settings).toBe(RECOVERY_SETTINGS);
      expect(loaded.startup).toEqual(state);
    }
  });

  it("чтение настроек упёрлось в повреждение: «повреждение найдено» вместо пустой страницы", async () => {
    const api = new MockApi();
    api.failNext("getSettings", "database_corrupt");
    const loaded = await loadStartup(api, NORMAL);
    expect(loaded.settings).toBe(RECOVERY_SETTINGS);
    expect(loaded.startup.corruptionFound).toBe(true);
  });

  it("любая другая ошибка чтения не выдаётся за повреждение", async () => {
    const api = new MockApi();
    api.failNext("getSettings", "store_read_failed");
    await expect(loadStartup(api, NORMAL)).rejects.toMatchObject({ code: "store_read_failed" });
  });
});

describe("AppRoot: повреждение найдено до первого кадра", () => {
  it("окно «База повреждена» с «Перезапустить» сразу, без журнала", async () => {
    const api = new MockApi();
    render(<AppRoot api={api} initialSettings={RECOVERY_SETTINGS} startup={{ ...NORMAL, corruptionFound: true }} />);
    expect(await screen.findByRole("main", { name: "База повреждена" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Перезапустить" })).toBeInTheDocument();
    expect(screen.queryByRole("textbox", { name: "Новая заметка" })).toBeNull();
  });
});
