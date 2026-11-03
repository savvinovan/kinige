interface Keyed {
  id: string;
  journalDate: string;
  createdAt: number;
}

/** Feed and cursor order (§4.1 of the spec): `(journal_date, created_at, id)`. */
function byFeedOrder(a: Keyed, b: Keyed): number {
  if (a.journalDate !== b.journalDate) return a.journalDate < b.journalDate ? -1 : 1;
  if (a.createdAt !== b.createdAt) return a.createdAt - b.createdAt;
  return a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
}

/**
 * Merges a fresh last page of the feed with the entries that are already loaded: everything the page covers is taken from it
 * (new entries appear, appended ones change, vanished ones disappear), while older entries that the person has already loaded
 * stay in place. If the page is the whole history (`hasOlder = false`), the feed equals the page.
 */
export function mergeLatestPage<M extends Keyed>(loaded: readonly M[], page: readonly M[], hasOlder: boolean): M[] {
  const oldest = page[0];
  if (!hasOlder || !oldest) return [...page];
  return [...loaded.filter((message) => byFeedOrder(message, oldest) < 0), ...page];
}

/**
 * Attaches to the loaded feed a page that came from the top (older entries) or from the bottom (newer ones, after jumping to an entry):
 * entries that are already there are not repeated (the Rust response is fresher and replaces the loaded copy), the order is the feed order.
 */
export function mergeOlder<M extends Keyed>(loaded: readonly M[], page: readonly M[]): M[] {
  const fresh = new Map(page.map((message) => [message.id, message]));
  const kept = loaded.map((message) => fresh.get(message.id) ?? message);
  const known = new Set(loaded.map((message) => message.id));
  return [...kept, ...page.filter((message) => !known.has(message.id))].sort(byFeedOrder);
}

/**
 * A feed page plus deleted entries that still stand in place muted ("Deleted · Restore"): the Rust response no longer
 * has them, and the person has not left the chat. Under a tag filter only entries with that tag remain. The order is the feed order.
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
