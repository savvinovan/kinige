import { beforeAll, describe, expect, it } from "vitest";
import type { Language } from "../../i18n";
import { buildEmojiData, createEmojiResolver, loadEmojiSource, type EmojiSource } from "./emojiData";
import { FALLBACK_SUPPORT } from "./emojiSupport";

// Настоящие наборы emojibase-data: проверяется то, что пикер действительно найдёт и покажет, а не придуманные фикстуры.
let sources: Record<Language, EmojiSource>;
beforeAll(async () => {
  sources = { en: await loadEmojiSource("en"), ru: await loadEmojiSource("ru") };
}, 30_000);

const labelsOf = (data: ReturnType<typeof buildEmojiData>) => data.emojis.map((emoji) => emoji.label);

/** Поиск как у frimousse (`searchEmojis`): подпись и теги, без учёта регистра. */
const found = (data: ReturnType<typeof buildEmojiData>, query: string) => {
  const text = query.toLowerCase().trim();
  return data.emojis.filter((emoji) => emoji.label.toLowerCase().includes(text) || emoji.tags.some((tag) => tag.toLowerCase().includes(text)));
};

describe("buildEmojiData", () => {
  it("подписи и категории — на языке интерфейса", () => {
    const ru = buildEmojiData("ru", sources, FALLBACK_SUPPORT);
    const en = buildEmojiData("en", sources, FALLBACK_SUPPORT);
    expect(ru.locale).toBe("ru");
    expect(ru.emojis.find((emoji) => emoji.emoji === "\u{1F436}")?.label).toBe("Морда собаки");
    expect(en.emojis.find((emoji) => emoji.emoji === "\u{1F436}")?.label).toBe("Dog face");
    expect(ru.categories.map((category) => category.label)).toContain("Предметы");
    expect(en.categories.map((category) => category.label)).toContain("Objects");
  });

  it("поиск находит эмодзи и по-русски, и по-английски при любом языке интерфейса", () => {
    for (const language of ["ru", "en"] as const) {
      const data = buildEmojiData(language, sources, FALLBACK_SUPPORT);
      expect(found(data, "собака").map((emoji) => emoji.emoji), `${language}: собака`).toContain("\u{1F436}");
      expect(found(data, "dog").map((emoji) => emoji.emoji), `${language}: dog`).toContain("\u{1F436}");
      expect(found(data, "КНИГА").map((emoji) => emoji.emoji), `${language}: КНИГА`).toContain("\u{1F4D5}");
      expect(found(data, "book").map((emoji) => emoji.emoji), `${language}: book`).toContain("\u{1F4D5}");
    }
  });

  it("служебная категория «component» и записи без категории отсечены", () => {
    const data = buildEmojiData("en", sources, FALLBACK_SUPPORT);
    expect(data.categories.map((category) => category.label)).not.toContain("Components");
    // Оттенки кожи (component) и региональные индикаторы не должны попасть в сетку.
    expect(data.emojis.some((emoji) => emoji.emoji === "\u{1F3FB}")).toBe(false);
    expect(data.emojis.some((emoji) => emoji.emoji === "\u{1F1E6}")).toBe(false);
  });

  it("эмодзи новее поддерживаемой версии скрыты, остальные на месте", () => {
    const all = buildEmojiData("en", sources, { version: 99, countryFlags: true });
    const old = buildEmojiData("en", sources, { version: 11, countryFlags: true });
    expect(old.emojis.length).toBeLessThan(all.emojis.length);
    expect(Math.max(...old.emojis.map((emoji) => emoji.version))).toBeLessThanOrEqual(11);
    expect(Math.max(...all.emojis.map((emoji) => emoji.version))).toBeGreaterThan(15.1);
  });

  it("флаги стран скрыты, если системный шрифт их не рисует", () => {
    const withFlags = buildEmojiData("en", sources, { version: 99, countryFlags: true });
    const without = buildEmojiData("en", sources, { version: 99, countryFlags: false });
    expect(withFlags.emojis.some((emoji) => emoji.countryFlag)).toBe(true);
    expect(without.emojis.some((emoji) => emoji.countryFlag)).toBe(false);
  });

  it("подписи уникальны (frimousse берёт подпись ключом строки списка)", () => {
    for (const language of ["ru", "en"] as const) {
      const labels = labelsOf(buildEmojiData(language, sources, { version: 99, countryFlags: true }));
      expect(new Set(labels).size, language).toBe(labels.length);
    }
  });

  it("оттенков кожи в данных нет: эмодзи остаётся с тоном по умолчанию", () => {
    const data = buildEmojiData("ru", sources, FALLBACK_SUPPORT);
    expect(data.emojis.every((emoji) => emoji.skins === undefined)).toBe(true);
    expect(data.emojis.find((emoji) => emoji.emoji === "\u{1F44B}")).toBeDefined();
  });
});

describe("createEmojiResolver", () => {
  it("отдаёт данные языка из бандла без сети", async () => {
    const resolve = createEmojiResolver(() => FALLBACK_SUPPORT);
    const data = await resolve("ru", {});
    expect(data.locale).toBe("ru");
    expect(data.emojis.length).toBeGreaterThan(1500);
    // Незнакомая локаль — английский.
    expect((await resolve("de", {})).locale).toBe("en");
  });

  it("отменённая загрузка отклоняется", async () => {
    const resolve = createEmojiResolver(() => FALLBACK_SUPPORT);
    const controller = new AbortController();
    controller.abort();
    await expect(resolve("en", { signal: controller.signal })).rejects.toBeDefined();
  });
});
