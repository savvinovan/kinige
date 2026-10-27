import type { EmojiData, EmojiDataResolver } from "frimousse";
import type { Language } from "../../i18n";
import { detectSupport, type EmojiSupport } from "./emojiSupport";

// Данные эмодзи лежат в бандле (пакет emojibase-data, MIT) и подгружаются динамическим import(): Vite делает из каждого
// JSON отдельный чанк, он грузится при первом открытии пикера. Сети нет: по умолчанию frimousse тянет данные с CDN,
// а CSP окна (`connect-src` без 'self') это запрещает, поэтому пикеру передаётся свой `resolveEmojiData`.

/** Запись emojibase `data.json`: только поля, которые нужны пикеру (остальные игнорируются). */
export interface RawEmoji {
  hexcode: string;
  emoji: string;
  label: string;
  tags?: readonly string[];
  /** Номер категории (`messages.groups[].order`); у служебных записей (региональные индикаторы) его нет. */
  group?: number;
  subgroup?: number;
  /** Версия Unicode Emoji, в которой эмодзи появился (0.6, 1, 15.1, 17 …). */
  version: number;
}

interface RawGroup {
  key: string;
  message: string;
  order: number;
}

/** `messages.json` emojibase: названия категорий и оттенков кожи на языке набора. */
export interface RawMessages {
  groups: readonly RawGroup[];
  subgroups: readonly RawGroup[];
  skinTones: readonly { key: string; message: string }[];
}

export interface EmojiSource {
  emojis: readonly RawEmoji[];
  messages: RawMessages;
}

type EmojiDataEmoji = EmojiData["emojis"][number];

const capitalize = (text: string) => text.charAt(0).toUpperCase() + text.slice(1);

const otherLanguage = (language: Language): Language => (language === "ru" ? "en" : "ru");

/**
 * Данные для frimousse на языке интерфейса. Подпись эмодзи и названия категорий — на языке интерфейса; в теги (по ним и
 * по подписи идёт поиск) добавлены подпись и теги второго языка, поэтому «собака» и «dog» находят один и тот же эмодзи
 * при любом языке интерфейса. Служебная категория «component», оттенки кожи, эмодзи новее `support.version` и (если
 * системный шрифт их не рисует) флаги стран отсечены.
 */
export function buildEmojiData(language: Language, sources: Record<Language, EmojiSource>, support: EmojiSupport): EmojiData {
  const own = sources[language];
  const other = sources[otherLanguage(language)];
  const twins = new Map(other.emojis.map((emoji) => [emoji.hexcode, emoji]));
  const component = own.messages.groups.find((group) => group.key === "component")?.order;
  const flagSubgroups = new Set(
    own.messages.subgroups.filter((subgroup) => subgroup.key === "country-flag" || subgroup.key === "subdivision-flag").map((subgroup) => subgroup.order),
  );

  const emojis: EmojiDataEmoji[] = [];
  for (const raw of own.emojis) {
    if (raw.group === undefined || raw.group === component || raw.version > support.version) continue;
    const countryFlag = raw.subgroup !== undefined && flagSubgroups.has(raw.subgroup);
    if (countryFlag && !support.countryFlags) continue;
    const twin = twins.get(raw.hexcode);
    const tags = [...new Set([...(raw.tags ?? []), ...(twin ? [twin.label, ...(twin.tags ?? [])] : [])])];
    emojis.push({
      emoji: raw.emoji,
      category: raw.group,
      label: capitalize(raw.label),
      version: raw.version,
      tags,
      countryFlag: countryFlag || undefined,
    });
  }

  return {
    locale: language,
    emojis,
    categories: own.messages.groups
      .filter((group) => group.key !== "component")
      .map((group) => ({ index: group.order, label: capitalize(group.message) })),
    skinTones: Object.fromEntries(own.messages.skinTones.map((tone) => [tone.key, capitalize(tone.message)])) as EmojiData["skinTones"],
  };
}

// Набор грузится один раз на язык. Типы JSON — из d.ts пакета (там тип из `emojibase`); здесь нужны только поля RawEmoji.
const loaders: Record<Language, () => Promise<EmojiSource>> = {
  en: async () => {
    const [data, messages] = await Promise.all([import("emojibase-data/en/data.json"), import("emojibase-data/en/messages.json")]);
    return { emojis: data.default as unknown as RawEmoji[], messages: messages.default as unknown as RawMessages };
  },
  ru: async () => {
    const [data, messages] = await Promise.all([import("emojibase-data/ru/data.json"), import("emojibase-data/ru/messages.json")]);
    return { emojis: data.default as unknown as RawEmoji[], messages: messages.default as unknown as RawMessages };
  },
};

const sources = new Map<Language, Promise<EmojiSource>>();

/** Набор emojibase языка; повторные вызовы получают тот же промис. Ошибка загрузки не кэшируется. */
export function loadEmojiSource(language: Language): Promise<EmojiSource> {
  let source = sources.get(language);
  if (!source) {
    source = loaders[language]();
    source.catch(() => sources.delete(language));
    sources.set(language, source);
  }
  return source;
}

/**
 * Резолвер данных для `resolveEmojiData` пикера. `support` решает, какие эмодзи рисует системный шрифт; в историях и тестах
 * он подменяется, в приложении — `detectSupport`.
 */
export function createEmojiResolver(support: (emojis: readonly RawEmoji[]) => EmojiSupport): EmojiDataResolver {
  return async (locale, { signal }) => {
    const language: Language = locale === "ru" ? "ru" : "en";
    const [own, other] = await Promise.all([loadEmojiSource(language), loadEmojiSource(otherLanguage(language))]);
    signal?.throwIfAborted();
    return buildEmojiData(language, { [language]: own, [otherLanguage(language)]: other } as Record<Language, EmojiSource>, support(own.emojis));
  };
}

/**
 * Единственный резолвер приложения. Функция должна быть одной и той же на всех рендерах: frimousse кэширует данные по
 * её идентичности.
 */
export const resolveEmojiData: EmojiDataResolver = createEmojiResolver(detectSupport);
