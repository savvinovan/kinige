/** Photos per grid row for an album of `count` photos (2..10, like Telegram's); more than ten are laid out three to a row. */
const ROWS: Readonly<Record<number, readonly number[]>> = {
  1: [1],
  2: [2],
  3: [1, 2],
  4: [2, 2],
  5: [2, 3],
  6: [3, 3],
  7: [2, 2, 3],
  8: [2, 3, 3],
  9: [3, 3, 3],
  10: [2, 2, 3, 3],
};

const WIDEST_ROW = 3;

/**
 * The rows of an album: the numbers add up to `count`, a row has one to three photos, the order is the order of the entry.
 * The shape is fixed by the count, so it never depends on the photos' sizes and the feed does not jump when they load.
 */
export function albumRows(count: number): readonly number[] {
  if (count <= 0) return [];
  const known = ROWS[count];
  if (known) return known;
  const rows = Array.from({ length: Math.floor(count / WIDEST_ROW) }, () => WIDEST_ROW);
  const rest = count % WIDEST_ROW;
  return rest === 0 ? rows : [...rows, rest];
}
