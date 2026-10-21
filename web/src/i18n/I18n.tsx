import { useMemo, type ReactNode } from "react";
import { I18nextProvider, useTranslation } from "react-i18next";
import { createI18n, type Language } from "./index";

/** Язык и переводы для всего поддерева. Экземпляр создаётся один раз на язык. */
export function I18n({ language, children }: { language: Language; children: ReactNode }) {
  const instance = useMemo(() => createI18n(language), [language]);
  return <I18nextProvider i18n={instance}>{children}</I18nextProvider>;
}

/** Текущий язык интерфейса: нужен там, где `Intl` форматирует дату или число. */
export function useLanguage(): Language {
  const { i18n } = useTranslation();
  return i18n.language === "ru" ? "ru" : "en";
}
