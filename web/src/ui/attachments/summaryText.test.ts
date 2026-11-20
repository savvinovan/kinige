import { describe, expect, it } from "vitest";
import { createI18n } from "../../i18n";
import { demoPhoto } from "./demo";
import { summaryOf, summaryText } from "./summaryText";

const en = createI18n("en").t;
const ru = createI18n("ru").t;
const of = (photos: number, files: number, name: string | null = null, voices = 0, voiceMs: number | null = null) => ({
  photos,
  files,
  voices,
  voiceMs,
  name,
  sticker: false,
  stickerEmoji: null,
});

describe("summaryText", () => {
  it("английский", () => {
    expect(summaryText(of(1, 0), en)).toBe("Photo");
    expect(summaryText(of(3, 0), en)).toBe("3 photos");
    expect(summaryText(of(0, 1, "menu.pdf"), en)).toBe("menu.pdf");
    expect(summaryText(of(0, 2), en)).toBe("2 files");
    expect(summaryText(of(2, 3), en)).toBe("5 attachments");
    expect(summaryText(of(1, 1), en)).toBe("2 attachments");
  });

  it("русский: формы числа", () => {
    expect(summaryText(of(1, 0), ru)).toBe("Фото");
    expect(summaryText(of(2, 0), ru)).toBe("2 фотографии");
    expect(summaryText(of(5, 0), ru)).toBe("5 фотографий");
    expect(summaryText(of(21, 0), ru)).toBe("21 фотография");
    expect(summaryText(of(0, 2), ru)).toBe("2 файла");
    expect(summaryText(of(0, 5), ru)).toBe("5 файлов");
    expect(summaryText(of(0, 1, "меню.pdf"), ru)).toBe("меню.pdf");
    expect(summaryText(of(2, 3), ru)).toBe("5 вложений");
    expect(summaryText(of(1, 1), ru)).toBe("2 вложения");
  });

  it("voice notes (slice 7a): one with its length, several as a count, mixed ones as attachments", () => {
    expect(summaryText(of(0, 0, "v.m4a", 1, 40_000), en)).toBe("Voice note · 0:40");
    expect(summaryText(of(0, 0, "v.m4a", 1, 3_723_000), en)).toBe("Voice note · 1:02:03");
    expect(summaryText(of(0, 0, "v.ogg", 1, null), en)).toBe("Voice note");
    expect(summaryText(of(0, 0, null, 3, 90_000), en)).toBe("3 voice notes");
    expect(summaryText(of(0, 0, "v.m4a", 1, 40_000), ru)).toBe("Голосовое · 0:40");
    expect(summaryText(of(0, 0, null, 2), ru)).toBe("2 голосовых сообщения");
    expect(summaryText(of(0, 0, null, 5), ru)).toBe("5 голосовых сообщений");
    expect(summaryText(of(1, 0, null, 1, 5_000), en)).toBe("2 attachments");
    expect(summaryText(of(0, 1, null, 1, 5_000), en)).toBe("2 attachments");
  });

  it("a sticker (slice 5c): 'Sticker 😺', without an emoji just 'Sticker'", () => {
    const sticker = { ...of(1, 0, "sticker.webp"), sticker: true, stickerEmoji: "\u{1F63A}" };
    expect(summaryText(sticker, en)).toBe("Sticker \u{1F63A}");
    expect(summaryText(sticker, ru)).toBe("Стикер \u{1F63A}");
    expect(summaryText({ ...sticker, stickerEmoji: null }, en)).toBe("Sticker");
    expect(summaryText({ ...sticker, stickerEmoji: null }, ru)).toBe("Стикер");
  });

  it("summaryOf mirrors Rust: only a single uncovered sticker marks the summary", () => {
    const sticker = { ...demoPhoto("s", "sticker.webp", 512, 512), sticker: true, stickerEmoji: "\u{1F63A}" };
    expect(summaryOf([sticker])).toMatchObject({ photos: 1, sticker: true, stickerEmoji: "\u{1F63A}" });
    expect(summaryOf([{ ...sticker, spoiler: true }])).toMatchObject({ photos: 1, sticker: false, stickerEmoji: null });
    expect(summaryOf([sticker, demoPhoto("p", "p.jpg", 10, 10)])).toMatchObject({ photos: 2, sticker: false });
    expect(summaryOf([{ ...sticker, sticker: false }])).toMatchObject({ sticker: false });
  });

  it("один файл без имени и запись без вложений", () => {
    expect(summaryText(of(0, 1), en)).toBe("File");
    expect(summaryText(of(0, 1, ""), ru)).toBe("Файл");
    expect(summaryText(of(0, 0), en)).toBe("");
  });
});
