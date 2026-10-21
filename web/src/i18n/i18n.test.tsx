import { I18nextProvider, useTranslation } from "react-i18next";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import en from "../locales/en.json";
import ru from "../locales/ru.json";
import { LANGUAGES, createI18n, type Language } from "./index";

type Tree = { [key: string]: string | Tree };

const PLURAL_SUFFIX = /_(zero|one|two|few|many|other)$/;

/** Ключи в виде «a.b.c»; у множественных форм суффикс отброшен, а категории собраны отдельно. */
function collect(tree: Tree, prefix = "", keys = new Map<string, Set<string>>()) {
  for (const [name, value] of Object.entries(tree)) {
    if (typeof value === "string") {
      const match = PLURAL_SUFFIX.exec(name);
      const base = prefix + name.replace(PLURAL_SUFFIX, "");
      const categories = keys.get(base) ?? new Set<string>();
      if (match?.[1]) categories.add(match[1]);
      keys.set(base, categories);
    } else collect(value, `${prefix}${name}.`, keys);
  }
  return keys;
}

describe("файлы переводов", () => {
  const files: Record<Language, Tree> = { en, ru };

  it("в en.json и ru.json одни и те же ключи", () => {
    expect([...collect(ru).keys()].sort()).toEqual([...collect(en).keys()].sort());
  });

  it.each(LANGUAGES)("%s: у каждой множественной формы есть все категории CLDR языка", (language) => {
    const required = new Intl.PluralRules(language).resolvedOptions().pluralCategories;
    for (const [key, categories] of collect(files[language])) {
      if (categories.size === 0) continue;
      expect([...categories].sort(), `${language}: ${key}`).toEqual([...required].sort());
    }
  });

  it("в строках нет пустых значений", () => {
    const empty = (tree: Tree): boolean =>
      Object.values(tree).some((value) => (typeof value === "string" ? value.trim() === "" : empty(value)));
    expect(empty(en)).toBe(false);
    expect(empty(ru)).toBe(false);
  });
});

describe("склонение", () => {
  const entries = (language: Language, count: number) => createI18n(language).t("count.entries", { count });

  it("ru: one, few, many, other по CLDR", () => {
    const expected: [number, string][] = [
      [0, "0 записей"],
      [1, "1 запись"],
      [2, "2 записи"],
      [5, "5 записей"],
      [11, "11 записей"],
      [21, "21 запись"],
      [22, "22 записи"],
      [25, "25 записей"],
      [111, "111 записей"],
    ];
    for (const [count, text] of expected) expect(entries("ru", count)).toBe(text);
  });

  it("en: one и other", () => {
    const expected: [number, string][] = [
      [0, "0 entries"],
      [1, "1 entry"],
      [2, "2 entries"],
      [11, "11 entries"],
      [21, "21 entries"],
    ];
    for (const [count, text] of expected) expect(entries("en", count)).toBe(text);
  });
});

describe("react-i18next", () => {
  function Hello() {
    const { t } = useTranslation();
    return <p>{t("composer.placeholder")}</p>;
  }

  it("синхронная инициализация: строка есть в первом же кадре, язык берётся из провайдера", () => {
    render(
      <I18nextProvider i18n={createI18n("ru")}>
        <Hello />
      </I18nextProvider>,
    );
    expect(screen.getByText("Написать заметку…")).toBeInTheDocument();
  });

  it("английский", () => {
    render(
      <I18nextProvider i18n={createI18n("en")}>
        <Hello />
      </I18nextProvider>,
    );
    expect(screen.getByText("Write a note…")).toBeInTheDocument();
  });
});
