import { describe, expect, it } from "vitest";
import { createI18n } from "../../i18n";
import { summaryText } from "./summaryText";

const en = createI18n("en").t;
const ru = createI18n("ru").t;
const of = (photos: number, files: number, name: string | null = null) => ({ photos, files, name });

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

  it("один файл без имени и запись без вложений", () => {
    expect(summaryText(of(0, 1), en)).toBe("File");
    expect(summaryText(of(0, 1, ""), ru)).toBe("Файл");
    expect(summaryText(of(0, 0), en)).toBe("");
  });
});
