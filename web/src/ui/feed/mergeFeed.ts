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
