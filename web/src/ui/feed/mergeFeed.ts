interface Keyed {
  id: string;
  journalDate: string;
  createdAt: number;
}

/** Порядок ленты и курсора (§4.1 ТЗ): `(journal_date, created_at, id)`. */
function byFeedOrder(a: Keyed, b: Keyed): number {
  if (a.journalDate !== b.journalDate) return a.journalDate < b.journalDate ? -1 : 1;
  if (a.createdAt !== b.createdAt) return a.createdAt - b.createdAt;
  return a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
}

/**
 * Сливает свежую последнюю страницу ленты с уже загруженными записями: всё, что страница покрывает, берётся из неё
 * (новые записи появляются, дописанные меняются, пропавшие исчезают), а более старые записи, которые человек уже подгрузил,
 * остаются на месте. Если страница — вся история (`hasOlder = false`), лента равна странице.
 */
export function mergeLatestPage<M extends Keyed>(loaded: readonly M[], page: readonly M[], hasOlder: boolean): M[] {
  const oldest = page[0];
  if (!hasOlder || !oldest) return [...page];
  return [...loaded.filter((message) => byFeedOrder(message, oldest) < 0), ...page];
}

/**
 * Присоединяет к загруженной ленте страницу, пришедшую сверху (старые записи) или снизу (новые, после перехода к записи):
 * записи, которые уже есть, не повторяются (ответ Rust свежее и заменяет загруженную копию), порядок — лентовый.
 */
export function mergeOlder<M extends Keyed>(loaded: readonly M[], page: readonly M[]): M[] {
  const fresh = new Map(page.map((message) => [message.id, message]));
  const kept = loaded.map((message) => fresh.get(message.id) ?? message);
  const known = new Set(loaded.map((message) => message.id));
  return [...kept, ...page.filter((message) => !known.has(message.id))].sort(byFeedOrder);
}

/**
 * Страница ленты плюс удалённые записи, которые ещё стоят на месте приглушёнными («Удалено · Вернуть»): в ответе Rust их
 * уже нет, а человек из чата не уходил. Под фильтром по тегу остаются только записи с этим тегом. Порядок — лентовый.
 */
export function withDeleted<M extends Keyed & { tags?: readonly string[] }>(
  loaded: readonly M[],
  page: readonly M[],
  deleted: ReadonlySet<string>,
  tag: string | null,
): M[] {
  const kept = loaded.filter(
    (message) =>
      deleted.has(message.id) &&
      !page.some((shown) => shown.id === message.id) &&
      (tag === null || (message.tags ?? []).includes(tag)),
  );
  return kept.length === 0 ? [...page] : [...page, ...kept].sort(byFeedOrder);
}
