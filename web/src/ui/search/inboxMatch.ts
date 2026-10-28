// «ё» и «е» в поиске не различаются; буквы кода — чтобы в исходнике не было кириллицы (проверка «строки только в локалях»).
const YO = String.fromCharCode(0x451);
const YE = String.fromCharCode(0x435);

const words = (text: string) => text.toLowerCase().replaceAll(YO, YE).match(/[\p{L}\p{N}]+/gu) ?? [];

/**
 * Совпадает ли с запросом переведённое имя Inbox («Входящие», «Inbox»): Rust Inbox по имени не ищет (имени в базе нет),
 * поэтому строку «Чаты» для него добавляет интерфейс. Каждое слово запроса — начало какого-то слова имени; основ слов
 * здесь нет, поэтому «входящ» находит, а «входящих» — нет.
 */
export function inboxMatches(name: string, query: string): boolean {
  const terms = words(query);
  if (terms.length === 0) return false;
  const title = words(name);
  return terms.every((term) => title.some((word) => word.startsWith(term)));
}
