import type { EmojiData, EmojiDataResolver } from "frimousse";
import type { Language } from "../../i18n";
import { detectSupport, type EmojiSupport } from "./emojiSupport";

// The emoji data lives in the bundle (the emojibase-data package, MIT) and is loaded by a dynamic import(): Vite makes a separate
// chunk from each JSON, it is loaded when the picker is first opened. There is no network: by default frimousse pulls data from a CDN,
// and the window's CSP (`connect-src` without 'self') forbids it, so the picker is given its own `resolveEmojiData`.

/** An emojibase `data.json` record: only the fields the picker needs (the rest are ignored). */
export interface RawEmoji {
  hexcode: string;
  emoji: string;
  label: string;
  tags?: readonly string[];
  /** Category number (`messages.groups[].order`); service records (regional indicators) do not have it. */
  group?: number;
  subgroup?: number;
  /** The Unicode Emoji version in which the emoji appeared (0.6, 1, 15.1, 17 ...). */
  version: number;
}

interface RawGroup {
  key: string;
  message: string;
  order: number;
}

/** emojibase `messages.json`: names of categories and skin tones in the language of the set. */
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
 * Data for frimousse in the interface language. The emoji label and category names are in the interface language; the label and tags of the
 * second language are added to the tags (search goes by them and by the label), so "собака" and "dog" find the same emoji
 * in either interface language. The service category "component", skin tones, emoji newer than `support.version` and (if
 * the system font does not draw them) country flags are cut off.
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

// The set is loaded once per language. JSON types come from the package d.ts (the type is from `emojibase` there); only the RawEmoji fields are needed here.
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

/** The emojibase set of the language; repeated calls get the same promise. A load error is not cached. */
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
 * Data resolver for the picker's `resolveEmojiData`. `support` decides which emoji the system font draws; in stories and tests
 * it is replaced, in the app it is `detectSupport`.
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
 * The app's only resolver. The function must be the same on all renders: frimousse caches the data by
 * its identity.
 */
export const resolveEmojiData: EmojiDataResolver = createEmojiResolver(detectSupport);
