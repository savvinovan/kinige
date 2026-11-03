import { useMemo, type ReactNode } from "react";
import { I18nextProvider, useTranslation } from "react-i18next";
import { createI18n, type Language } from "./index";

/** The language and translations for the whole subtree. The instance is created once per language. */
export function I18n({ language, children }: { language: Language; children: ReactNode }) {
  const instance = useMemo(() => createI18n(language), [language]);
  return <I18nextProvider i18n={instance}>{children}</I18nextProvider>;
}

/** The current interface language: needed where `Intl` formats a date or a number. */
export function useLanguage(): Language {
  const { i18n } = useTranslation();
  return i18n.language === "ru" ? "ru" : "en";
}
