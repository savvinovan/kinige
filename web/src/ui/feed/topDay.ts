/**
 * The journal day at the top of a scrolled feed (slice 6): the first element carrying `data-journal-date` (a day label or an
 * entry) whose bottom edge is below the top edge of the scroller. The elements are in document order, which is top-to-bottom
 * order, so a binary search finds it in a few layout reads even in a long feed. `null` for an empty or fully scrolled-past feed.
 */
export function topDayOf(scroller: HTMLElement): string | null {
  const nodes = scroller.querySelectorAll<HTMLElement>("[data-journal-date]");
  const top = scroller.getBoundingClientRect().top;
  let low = 0;
  let high = nodes.length - 1;
  let found = -1;
  while (low <= high) {
    const mid = (low + high) >> 1;
    const node = nodes[mid];
    if (node && node.getBoundingClientRect().bottom > top) {
      found = mid;
      high = mid - 1;
    } else {
      low = mid + 1;
    }
  }
  return nodes[found]?.dataset.journalDate ?? null;
}
